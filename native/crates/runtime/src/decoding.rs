//! Reading a value out of a document: what a generated reader asks of the place it stands at, what
//! it records where the place is not what the declaration says, and what a host reads back once
//! the reading is done.
//!
//! Nothing here knows a declaration. Which members a value has, which of them may be absent, where
//! a set of alternatives says which case it is and what a clause is called all stay in the code a
//! compile generates per declaration; this answers the questions that code asks of one place — is it
//! an object, what is under this key, is it an `Int` — and keeps what was found wrong. A scalar is
//! read here and not in generated code, the way a string is compared here: what a place written as
//! `1.0` is, is one answer for every declaration and not one emitted into every reader.
//!
//! What was found wrong is kept, all of it, as issues with Raoh's codes: a caller who fixes one
//! field and is then told about the next has been made to ask as many times as its document had
//! mistakes. An issue carries a code, the JSON Pointer of where it was found, and what else its code
//! says about it as named entries, which is what Raoh's own metadata is. Not a sentence: what a
//! person reads is written against the code by whoever shows it.
//!
//! Everything a reading makes that outlives it — the reading itself, its issues, its paths and the
//! text in them — is taken from the arena, so the mark a caller took before decoding drops it with
//! everything else the call made. The document is the one thing on the heap, and the reading drops
//! it when it ends.

use crate::amount::Amount;
use crate::decimal::{Decimal, decimal_of};
use crate::document::{Form, Node, parsed};
use crate::temporal::{
    Date, DateTime, Instant, Time, date_of, date_time_of, instant_of, parse_date, parse_date_time,
    parse_instant, parse_time, time_of,
};
use crate::{Count, Text, Value, souther_alloc, string_of, text};
use souther_native_abi::{DECODED_ISSUES, DECODED_MALFORMED, DECODED_VALUE};
use std::ptr;

/// One reading of one document, from when its bytes are handed over to what a host is answered.
#[repr(C)]
pub struct Decoding {
    /// The tree, until the reading ends; null from then on, and from the start where the bytes
    /// were not a document.
    document: *mut Node,
    /// Where the bytes stopped being a document, or below nought where they are one.
    malformed_at: i64,
    first: *mut Issue,
    last: *mut Issue,
    count: i64,
    /// Every issue in the order it was found, laid out once the reading ends so a host reaches the
    /// n-th without walking the n before it.
    issues: *const *const Issue,
    value: *const u8,
}

/// Where in a document a place is: the place it is below, and the key or index it is below it by.
/// The document's root is no path at all, a null.
///
/// Made a step at a time as a reader walks down, and written out as a JSON Pointer only when an
/// issue is found there, so a document read without a mistake writes none.
#[repr(C)]
pub struct Path {
    above: *const Path,
    /// A string of the runtime's layout.
    step: *const u8,
}

/// What entries an issue can carry. Raoh's metadata is a map, and the most any code here says is
/// the three an invariant does.
const META: usize = 3;

/// One thing found wrong.
#[repr(C)]
pub struct Issue {
    code: *const u8,
    path: *const u8,
    meta: [(*const u8, *const u8); META],
    meta_count: i64,
    next: *mut Issue,
}

/// Room in the arena for one `T`, written with `value`.
fn held<T>(value: T) -> *mut T {
    let at = souther_alloc(Count(size_of::<T>() as i64)).cast::<T>();
    // The arena answers room aligned to a slot, which is as aligned as anything here asks.
    const { assert!(align_of::<T>() <= souther_native_abi::SLOT as usize) };
    unsafe { at.write(value) };
    at
}

/// The JSON Pointer of `path`: each step after a `/`, with `~` written `~0` and `/` written `~1`
/// so that a key holding either is one step and not two (RFC 6901).
///
/// # Safety
/// `path` is null or a path made here whose steps are strings of the runtime's layout.
unsafe fn pointer(path: *const Path) -> String {
    let mut steps = Vec::new();
    let mut at = path;
    while let Some(step) = unsafe { at.as_ref() } {
        steps.push(unsafe { text(&step.step) }.as_str());
        at = step.above;
    }
    let mut written = String::new();
    for step in steps.iter().rev() {
        written.push('/');
        for character in step.chars() {
            match character {
                '~' => written.push_str("~0"),
                '/' => written.push_str("~1"),
                other => written.push(other),
            }
        }
    }
    written
}

/// A string of the runtime's layout holding this text, as an issue holds one.
fn string(text: &str) -> *const u8 {
    string_of(text).cast()
}

/// Records an issue found at `path`.
///
/// # Safety
/// `decoding` is one [`souther_decode_begin`] answered and still reading, and `path` is as
/// [`pointer`] says.
unsafe fn found(decoding: *mut Decoding, code: &str, path: *const Path, meta: &[(&str, &str)]) {
    assert!(meta.len() <= META, "no code says more than {META} things");
    let mut entries = [(ptr::null::<u8>(), ptr::null::<u8>()); META];
    for (entry, (key, value)) in entries.iter_mut().zip(meta) {
        *entry = (string(key), string(value));
    }
    let issue = held(Issue {
        code: string(code),
        path: string(&unsafe { pointer(path) }),
        meta: entries,
        meta_count: meta.len() as i64,
        next: ptr::null_mut(),
    });
    let decoding = unsafe { &mut *decoding };
    match unsafe { decoding.last.as_mut() } {
        None => decoding.first = issue,
        Some(last) => last.next = issue,
    }
    decoding.last = issue;
    decoding.count += 1;
}

/// A place was something other than what was declared there.
unsafe fn mismatched(decoding: *mut Decoding, path: *const Path, node: &Node, wanted: &str) {
    unsafe {
        found(
            decoding,
            "type_mismatch",
            path,
            &[("actual", node.kind()), ("expected", wanted)],
        )
    };
}

/// Text arriving from outside, admitted where it arrives, which is at its string leaf: put in NFC
/// (spec §string-canonical). What most documents write is ASCII, which is NFC already and is taken
/// as it is.
fn canonical(written: &[u8]) -> std::borrow::Cow<'_, str> {
    souther_text::admitted(written).expect("the parser holds a string to be UTF-8")
}

/// Begins reading `length` bytes at `bytes` as a document in the external form.
///
/// # Safety
/// `bytes` points at `length` bytes that may be read, for as long as this call runs: nothing
/// after it reads them.
/// # Panics
/// Where the length is below nought.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decode_begin(bytes: *const u8, length: Count) -> *mut Decoding {
    unsafe { begun(bytes, length, Form::Text) }
}

/// Begins reading `length` bytes at `bytes` as a value a host built of ordered maps, every one of
/// them written as an object keyed as the host keyed it ([`Form::HostValue`]).
///
/// The same reading as [`souther_decode_begin`] in every place but one: where the reader takes an
/// array, a map whose keys are its indices is one, since that is what a list is to such a host.
/// So a host whose empty list is its empty object hands over either and is read as whichever the
/// position holds.
///
/// # Safety
/// As [`souther_decode_begin`].
/// # Panics
/// As [`souther_decode_begin`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decode_host_begin(
    bytes: *const u8,
    length: Count,
) -> *mut Decoding {
    unsafe { begun(bytes, length, Form::HostValue) }
}

/// # Safety
/// As [`souther_decode_begin`].
unsafe fn begun(bytes: *const u8, length: Count, form: Form) -> *mut Decoding {
    let length =
        usize::try_from(length.0).expect("a document is handed over as bytes, never fewer");
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, length) }
    };
    let (document, malformed_at) = match parsed(bytes, form) {
        Ok(root) => (Box::into_raw(Box::new(root)), -1),
        Err(malformed) => (ptr::null_mut(), malformed.at as i64),
    };
    held(Decoding {
        document,
        malformed_at,
        first: ptr::null_mut(),
        last: ptr::null_mut(),
        count: 0,
        issues: ptr::null(),
        value: ptr::null(),
    })
}

/// The document's root, or null where the bytes were not a document and there is nothing to read.
///
/// # Safety
/// `decoding` is one [`souther_decode_begin`] answered and still reading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decode_root(decoding: *const Decoding) -> *const Node {
    unsafe { (*decoding).document }
}

fn dropped(decoding: &mut Decoding) {
    if !decoding.document.is_null() {
        drop(unsafe { Box::from_raw(decoding.document) });
        decoding.document = ptr::null_mut();
    }
}

/// Ends a reading with what it read: `value`, or null where it read none, which it may only where
/// the bytes were not a document or an issue was found.
///
/// # Safety
/// `decoding` is one [`souther_decode_begin`] answered and still reading.
/// # Panics
/// Where a document with nothing wrong in it was read as no value, which is the generated reader
/// having lost one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decode_end(decoding: *mut Decoding, value: *const Value) {
    let decoding = unsafe { &mut *decoding };
    dropped(decoding);
    if decoding.malformed_at < 0 && decoding.count == 0 {
        assert!(
            !value.is_null(),
            "a document with nothing wrong in it reads as a value"
        );
        decoding.value = value.cast();
    }
    let count = decoding.count as usize;
    let issues =
        souther_alloc(Count((count * size_of::<*const Issue>()) as i64)).cast::<*const Issue>();
    let mut at = decoding.first;
    for n in 0..count {
        unsafe { issues.add(n).write(at) };
        at = unsafe { (*at).next };
    }
    decoding.issues = issues;
}

/// Ends a reading that will answer nothing: a clause it ran ended without a value, and what the
/// caller is answered is that status and not this reading.
///
/// # Safety
/// `decoding` is one [`souther_decode_begin`] answered and still reading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decode_abandon(decoding: *mut Decoding) {
    dropped(unsafe { &mut *decoding });
}

/// The place `step` below `path`.
///
/// # Safety
/// `path` is null or one this answered, and `step` is a string of the runtime's layout that lasts
/// as long as the path does.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_path_below(path: *const Path, step: *const Text) -> *const Path {
    held(Path {
        above: path,
        step: step.cast(),
    })
}

/// The place of the element at `index` below `path`, the index written as a JSON Pointer writes
/// one: in decimal, with no sign and no leading nought.
///
/// Here and not in generated code, which would otherwise be writing a number out as text for a
/// path that is only ever written out where an issue is found.
///
/// # Safety
/// `path` is null or one this or [`souther_path_below`] answered; `index` is not negative.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_path_at(path: *const Path, index: Count) -> *const Path {
    held(Path {
        above: path,
        step: string(&index.0.to_string()),
    })
}

/// Whether `node` is an array, having recorded that it is not where it is not.
///
/// # Safety
/// As [`souther_read_object`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_array(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
) -> i8 {
    let node = unsafe { &*node };
    if node.length().is_some() {
        return 1;
    }
    unsafe { mismatched(decoding, path, node, "an array") };
    0
}

/// How many elements the array `node` holds.
///
/// # Safety
/// `node` is a place in a document being read, which [`souther_read_array`] answered is an array.
/// # Panics
/// Where it is not an array, which is generated code asking without having asked that first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_array_length(node: *const Node) -> Count {
    let length = unsafe { &*node }
        .length()
        .expect("the length of a place that is not an array");
    Count(length as i64)
}

/// The element of the array `node` at `index`.
///
/// # Safety
/// As [`souther_read_array_length`], and `index` is below the length it answered.
/// # Panics
/// Where it is not an array, or the index is outside it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_element(node: *const Node, index: Count) -> *const Node {
    let at = usize::try_from(index.0).expect("an index is not negative");
    unsafe { &*node }
        .element(at)
        .expect("an element of a place that is not an array, or past its end")
}

/// Whether `node` is an object, having recorded that it is not where it is not.
///
/// # Safety
/// `node` is a place in the document `decoding` is reading; `path` is where it is.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_object(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
) -> i8 {
    let node = unsafe { &*node };
    if node.is_object() {
        return 1;
    }
    unsafe { mismatched(decoding, path, node, "an object") };
    0
}

/// The member of the object `node` under `key`, or null where the object has none.
///
/// # Safety
/// `node` is a place in a document being read; `key` is a string of the runtime's layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_member(node: *const Node, key: *const Text) -> *const Node {
    match unsafe { (*node).member(text(&key).as_bytes()) } {
        Some(member) => member,
        None => ptr::null(),
    }
}

/// Records that a field the declaration says every value has was not written, at `path`, which
/// is where it would have been.
///
/// # Safety
/// As [`souther_read_object`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_missing(path: *const Path, decoding: *mut Decoding) {
    unsafe {
        found(
            decoding,
            "missing_field",
            path,
            &[("actual", "nothing"), ("expected", "a field")],
        )
    };
}

/// Whether `node` is `null`, which is what absence is written as where there is no key to leave
/// out.
///
/// # Safety
/// `node` is a place in a document being read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_null(node: *const Node) -> i8 {
    i8::from(matches!(unsafe { &*node }, Node::Null))
}

/// Writes what a scalar reader read through `out`, and answers whether it read one.
///
/// Every scalar reader writes its room whatever it answers: the value, or `none` where it read
/// nothing and recorded why. So generated code never holds what a stack slot happened to hold
/// before the call, whichever way the call went — the same holds of a reader of a declared type,
/// which writes its value or nothing whenever it answers — and the rule is kept here, once, rather
/// than by each reader remembering it.
unsafe fn answered<T>(out: *mut T, read: Option<T>, none: T) -> i8 {
    let there = read.is_some();
    unsafe { out.write(read.unwrap_or(none)) };
    i8::from(there)
}

/// An `Int`, written through `out`, where `node` writes one: a whole number, as digits with no
/// point and no exponent, within sixty-four bits. Nought is written where it does not.
///
/// # Safety
/// As [`souther_read_object`], and `out` may be written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_int(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    out: *mut i64,
) -> i8 {
    let read = unsafe { int(&*node, path, decoding) };
    unsafe { answered(out, read, 0) }
}

unsafe fn int(node: &Node, path: *const Path, decoding: *mut Decoding) -> Option<i64> {
    let Node::Number(written) = node else {
        unsafe { mismatched(decoding, path, node, "Int") };
        return None;
    };
    let (negative, digits) = match written.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, &written[..]),
    };
    // A point or an exponent means the document wrote an amount and not a whole number, whatever
    // amount it is.
    if !digits.iter().all(u8::is_ascii_digit) {
        unsafe { mismatched(decoding, path, node, "Int") };
        return None;
    }
    let mut magnitude: i128 = 0;
    for &digit in digits {
        magnitude = magnitude * 10 + i128::from(digit - b'0');
        if magnitude > 1 << 63 {
            break;
        }
    }
    let value = if negative { -magnitude } else { magnitude };
    let read = i64::try_from(value).ok();
    if read.is_none() {
        unsafe {
            found(
                decoding,
                "out_of_range",
                path,
                &[("actual", "number"), ("expected", "Int")],
            )
        };
    }
    read
}

/// A `Bool`, written through `out` as nought or one, where `node` writes one; nought where it does
/// not.
///
/// # Safety
/// As [`souther_read_int`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_bool(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    out: *mut i8,
) -> i8 {
    let read = match unsafe { &*node } {
        Node::Bool(truth) => Some(i8::from(*truth)),
        other => {
            unsafe { mismatched(decoding, path, other, "Bool") };
            None
        }
    };
    unsafe { answered(out, read, 0) }
}

/// A `String`, written through `out` as a string of the runtime's layout in the arena, where
/// `node` writes one: its text canonicalized to NFC. Null is written where it does not.
///
/// # Safety
/// As [`souther_read_int`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_string(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    out: *mut *mut Text,
) -> i8 {
    let read = match unsafe { &*node } {
        Node::String(written) => Some(string_of(&canonical(written))),
        other => {
            unsafe { mismatched(decoding, path, other, "String") };
            None
        }
    };
    unsafe { answered(out, read, ptr::null_mut()) }
}

/// A `Decimal`, written through `out` as one of the runtime's in the arena, where `node` writes a
/// number whose scale is one a `Decimal` has: the value its spelling writes, at the scale its
/// spelling gives it, as many places as its fraction has less its exponent. So `1.50` is read at
/// scale 2 and `1e2` at scale -2, as the JVM reads them. Null is written where it does not.
///
/// A number whose exponent puts its scale outside the 32-bit range is `out_of_range`: a number, and
/// not one a `Decimal` holds, as an `Int` too wide for sixty-four bits is.
///
/// # Safety
/// As [`souther_read_int`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_decimal(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    out: *mut *mut Decimal,
) -> i8 {
    let read = match unsafe { &*node } {
        Node::Number(written) => {
            let read = Amount::of_json_number(written);
            if read.is_none() {
                unsafe {
                    found(
                        decoding,
                        "out_of_range",
                        path,
                        &[("actual", "number"), ("expected", "Decimal")],
                    )
                };
            }
            read.as_ref().map(decimal_of)
        }
        other => {
            unsafe { mismatched(decoding, path, other, "Decimal") };
            None
        }
    };
    unsafe { answered(out, read, ptr::null_mut()) }
}

/// The text at `node`, where it is a string, which is where a temporal is written: every temporal
/// is text at a boundary, and one that is not is a string that was not there.
unsafe fn temporal_text<'a>(
    node: &'a Node,
    path: *const Path,
    decoding: *mut Decoding,
) -> Option<std::borrow::Cow<'a, str>> {
    match node {
        Node::String(written) => Some(canonical(written)),
        other => {
            unsafe { mismatched(decoding, path, other, "String") };
            None
        }
    }
}

/// A place whose text is no value of a temporal type, or one a type held to the second would have
/// to round.
unsafe fn refused(decoding: *mut Decoding, path: *const Path) {
    unsafe { found(decoding, "invalid_format", path, &[]) };
}

/// A `Date`, written through `out` as one of the runtime's in the arena, where `node` is text that
/// names one: `yyyy-MM-dd` with the year as `LocalDate.toString` writes it. Null is written where
/// it is not.
///
/// # Safety
/// As [`souther_read_int`], and `out` is room for the address of a `Date`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_date(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    out: *mut *mut Date,
) -> i8 {
    let read = unsafe { temporal_text(&*node, path, decoding) }.and_then(|written| {
        let day = parse_date(written.as_bytes());
        if day.is_none() {
            unsafe { refused(decoding, path) };
        }
        day.map(date_of)
    });
    unsafe { answered(out, read, ptr::null_mut()) }
}

/// A `Time`, as [`souther_read_date`], from `HH:mm` or `HH:mm:ss`. A fraction of a second that is
/// not nought is refused and not dropped (spec §a-local-temporal-is-held-to-the-second).
///
/// # Safety
/// As [`souther_read_date`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_time(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    out: *mut *mut Time,
) -> i8 {
    let read = unsafe { temporal_text(&*node, path, decoding) }.and_then(|written| {
        let second = parse_time(written.as_bytes());
        if second.is_err() {
            unsafe { refused(decoding, path) };
        }
        second.ok().map(time_of)
    });
    unsafe { answered(out, read, ptr::null_mut()) }
}

/// A `DateTime`, as [`souther_read_time`], from a date, a `T` and a time.
///
/// # Safety
/// As [`souther_read_date`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_datetime(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    out: *mut *mut DateTime,
) -> i8 {
    let read = unsafe { temporal_text(&*node, path, decoding) }.and_then(|written| {
        let second = parse_date_time(written.as_bytes());
        if second.is_err() {
            unsafe { refused(decoding, path) };
        }
        second.ok().map(date_time_of)
    });
    unsafe { answered(out, read, ptr::null_mut()) }
}

/// An `Instant`, as [`souther_read_date`], from text written in UTC or from an offset, which is
/// read as the moment it names (spec §an-instant-carries-what-a-timestamp-said). A leap second is
/// refused.
///
/// # Safety
/// As [`souther_read_date`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_instant(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    out: *mut *mut Instant,
) -> i8 {
    let read = unsafe { temporal_text(&*node, path, decoding) }.and_then(|written| {
        let moment = parse_instant(written.as_bytes());
        if moment.is_none() {
            unsafe { refused(decoding, path) };
        }
        moment.map(|(second, nano)| instant_of(second, nano))
    });
    unsafe { answered(out, read, ptr::null_mut()) }
}

/// Whether `node` is text naming a case, having recorded that it is not where it is not. Which
/// case it names is asked of it next ([`souther_read_is`]).
///
/// # Safety
/// As [`souther_read_object`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_case(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
) -> i8 {
    let node = unsafe { &*node };
    if let Node::String(_) = node {
        return 1;
    }
    unsafe { mismatched(decoding, path, node, "a case") };
    0
}

/// What the object `node` names its case with, under `key`, and null having recorded why where it
/// names none: the key is not there, or what is under it is not text. `path` is where the key's
/// member is.
///
/// # Safety
/// As [`souther_read_object`], and `key` is a string of the runtime's layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_tag(
    node: *const Node,
    key: *const Text,
    path: *const Path,
    decoding: *mut Decoding,
) -> *const Node {
    let Some(tag) = (unsafe { (*node).member(text(&key).as_bytes()) }) else {
        unsafe {
            found(
                decoding,
                "missing_field",
                path,
                &[("actual", "nothing"), ("expected", "a case")],
            )
        };
        return ptr::null();
    };
    if unsafe { souther_read_case(tag, path, decoding) } == 0 {
        return ptr::null();
    }
    tag
}

/// Whether the text `node` writes is `name`, once it is canonicalized as any text arriving is.
///
/// # Safety
/// `node` is a place in a document being read that holds text; `name` is a string of the
/// runtime's layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_is(node: *const Node, name: *const Text) -> i8 {
    match unsafe { &*node } {
        Node::String(written) => {
            i8::from(canonical(written).as_bytes() == unsafe { text(&name) }.as_bytes())
        }
        _ => 0,
    }
}

/// Records that the text `node` writes names none of the cases there are, at `path`.
///
/// # Safety
/// As [`souther_read_object`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_not_a_case(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
) {
    // What was written, as the text it is admitted as: every string holds text in NFC.
    let written = match unsafe { &*node } {
        Node::String(written) => canonical(written),
        _ => std::borrow::Cow::Borrowed(""),
    };
    unsafe {
        found(
            decoding,
            "not_allowed",
            path,
            &[("actual", &written), ("expected", "a case")],
        )
    };
}

/// Records that a value read at `path` breaks a clause its type holds its values to: the type as
/// its module and its name, and the clause's name where the author gave it one, `clause` being null
/// where they did not (spec §decoder-error).
///
/// # Safety
/// As [`souther_read_object`]; `module` and `name` are strings of the runtime's layout, and so is
/// `clause` where it is not null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_invariant(
    path: *const Path,
    decoding: *mut Decoding,
    module: *const Text,
    name: *const Text,
    clause: *const Text,
) {
    let (module, name) = unsafe { (text(&module).as_str(), text(&name).as_str()) };
    let mut meta: Vec<(&str, &str)> = vec![("module", module), ("type", name)];
    if !clause.is_null() {
        meta.push(("clause", unsafe { text(&clause).as_str() }));
    }
    unsafe { found(decoding, "invariant_violation", path, &meta) };
}

/// What a reading came to: [`DECODED_VALUE`], [`DECODED_ISSUES`] or [`DECODED_MALFORMED`].
///
/// # Safety
/// `decoded` is a reading a decoder answered, and the mark below it still stands.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decoded_outcome(decoded: *const Decoding) -> i32 {
    let decoded = unsafe { &*decoded };
    if decoded.malformed_at >= 0 {
        DECODED_MALFORMED
    } else if decoded.count > 0 {
        DECODED_ISSUES
    } else {
        DECODED_VALUE
    }
}

/// The value read, where the reading came to one; null where it did not.
///
/// # Safety
/// As [`souther_decoded_outcome`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decoded_value(decoded: *const Decoding) -> *const Value {
    unsafe { (*decoded).value.cast() }
}

/// The offset of the byte the document stopped being one at, where it was not one; below nought
/// where it was.
///
/// # Safety
/// As [`souther_decoded_outcome`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decoded_malformed_at(decoded: *const Decoding) -> Count {
    Count(unsafe { (*decoded).malformed_at })
}

/// How many issues the reading found.
///
/// # Safety
/// As [`souther_decoded_outcome`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decoded_issue_count(decoded: *const Decoding) -> Count {
    Count(unsafe { (*decoded).count })
}

/// The issue at `at`, counting from nought in the order they were found.
///
/// # Safety
/// As [`souther_decoded_outcome`].
/// # Panics
/// Where there is no issue at `at`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decoded_issue(
    decoded: *const Decoding,
    at: Count,
) -> *const Issue {
    let Count(at) = at;
    let decoded = unsafe { &*decoded };
    assert!(
        (0..decoded.count).contains(&at),
        "an issue is asked for by where it stands among the {} there are",
        decoded.count
    );
    unsafe { *decoded.issues.add(at as usize) }
}

/// The issue's code, as a string of the runtime's layout.
///
/// # Safety
/// `issue` is one [`souther_decoded_issue`] answered, and the mark below it still stands.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_issue_code(issue: *const Issue) -> *const Text {
    unsafe { (*issue).code.cast() }
}

/// Where the issue was found, as a JSON Pointer in a string of the runtime's layout: empty for the
/// document's root.
///
/// # Safety
/// As [`souther_issue_code`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_issue_path(issue: *const Issue) -> *const Text {
    unsafe { (*issue).path.cast() }
}

/// How many named entries the issue carries besides its code and its path.
///
/// # Safety
/// As [`souther_issue_code`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_issue_meta_count(issue: *const Issue) -> Count {
    Count(unsafe { (*issue).meta_count })
}

fn entry(issue: &Issue, at: i64) -> (*const u8, *const u8) {
    assert!(
        (0..issue.meta_count).contains(&at),
        "an entry is asked for by where it stands among the {} there are",
        issue.meta_count
    );
    issue.meta[at as usize]
}

/// The name of the issue's entry at `at`.
///
/// # Safety
/// As [`souther_issue_code`].
/// # Panics
/// Where there is no entry at `at`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_issue_meta_key(issue: *const Issue, at: Count) -> *const Text {
    entry(unsafe { &*issue }, at.0).0.cast()
}

/// What the issue's entry at `at` says.
///
/// # Safety
/// As [`souther_issue_meta_key`].
/// # Panics
/// Where there is no entry at `at`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_issue_meta_value(issue: *const Issue, at: Count) -> *const Text {
    entry(unsafe { &*issue }, at.0).1.cast()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{souther_mark, souther_reset, souther_string_of_utf8};

    fn said(at: *const Text) -> String {
        String::from_utf8(unsafe { text(&at).as_bytes() }.to_vec()).unwrap()
    }

    fn literal(value: &str) -> *mut Text {
        unsafe { souther_string_of_utf8(value.as_ptr(), Count(value.len() as i64)) }
    }

    fn begun(document: &str) -> *mut Decoding {
        unsafe { souther_decode_begin(document.as_ptr(), Count(document.len() as i64)) }
    }

    /// Every issue a reading found, as `path code key=value…`.
    fn issues(decoding: *mut Decoding) -> Vec<String> {
        unsafe {
            souther_decode_end(decoding, ptr::null());
            (0..souther_decoded_issue_count(decoding).0)
                .map(|at| {
                    let issue = souther_decoded_issue(decoding, Count(at));
                    let mut line = format!(
                        "{} {}",
                        said(souther_issue_path(issue)),
                        said(souther_issue_code(issue))
                    );
                    for entry in 0..souther_issue_meta_count(issue).0 {
                        line.push_str(&format!(
                            " {}={}",
                            said(souther_issue_meta_key(issue, Count(entry))),
                            said(souther_issue_meta_value(issue, Count(entry)))
                        ));
                    }
                    line
                })
                .collect()
        }
    }

    fn int(document: &str) -> Result<i64, Vec<String>> {
        let decoding = begun(document);
        let mut out = 0;
        let read = unsafe {
            souther_read_int(
                souther_decode_root(decoding),
                ptr::null(),
                decoding,
                &mut out,
            )
        };
        if read == 1 {
            unsafe { souther_decode_abandon(decoding) };
            Ok(out)
        } else {
            Err(issues(decoding))
        }
    }

    /// A reader that read nothing still writes its room, so what the caller's room held before
    /// the call is never taken for anything.
    #[test]
    fn a_scalar_reader_writes_its_room_whether_it_read_one_or_not() {
        let mark = souther_mark();
        let decoding = begun("{}");
        let root = unsafe { souther_decode_root(decoding) };
        let mut int = -7;
        let mut truth = 7;
        let mut text = literal("before");
        unsafe {
            assert_eq!(souther_read_int(root, ptr::null(), decoding, &mut int), 0);
            assert_eq!(
                souther_read_bool(root, ptr::null(), decoding, &mut truth),
                0
            );
            assert_eq!(
                souther_read_string(root, ptr::null(), decoding, &mut text),
                0
            );
            souther_decode_abandon(decoding);
        }
        assert_eq!((int, truth, text), (0, 0, ptr::null_mut()));
        souther_reset(mark);
    }

    #[test]
    fn an_int_is_a_whole_number_within_sixty_four_bits() {
        let mark = souther_mark();
        assert_eq!(int("42"), Ok(42));
        assert_eq!(int("-0"), Ok(0));
        assert_eq!(int("9223372036854775807"), Ok(i64::MAX));
        assert_eq!(int("-9223372036854775808"), Ok(i64::MIN));
        assert_eq!(
            int("9223372036854775808"),
            Err(vec![" out_of_range actual=number expected=Int".to_string()])
        );
        assert_eq!(
            int("-99999999999999999999999"),
            Err(vec![" out_of_range actual=number expected=Int".to_string()])
        );
        assert_eq!(
            int("1.0"),
            Err(vec![
                " type_mismatch actual=number expected=Int".to_string()
            ])
        );
        assert_eq!(
            int("1e0"),
            Err(vec![
                " type_mismatch actual=number expected=Int".to_string()
            ])
        );
        assert_eq!(
            int("\"1\""),
            Err(vec![
                " type_mismatch actual=string expected=Int".to_string()
            ])
        );
        souther_reset(mark);
    }

    /// A key holding `/` or `~` is one step of the pointer and not two (RFC 6901).
    #[test]
    fn a_path_is_a_json_pointer_with_its_steps_escaped() {
        let mark = souther_mark();
        let decoding = begun("{}");
        let path = unsafe {
            let a = souther_path_below(ptr::null(), literal("a/b"));
            let b = souther_path_below(a, literal("~c"));
            souther_path_below(b, literal("0"))
        };
        unsafe { souther_read_missing(path, decoding) };
        assert_eq!(
            issues(decoding),
            vec!["/a~1b/~0c/0 missing_field actual=nothing expected=a field"]
        );
        souther_reset(mark);
    }

    /// An element's place is its index below the array's, and a place that is not an array is
    /// recorded as one.
    #[test]
    fn an_array_is_read_element_by_element_at_its_index() {
        let mark = souther_mark();
        let decoding = begun("[1, true]");
        let root = unsafe { souther_decode_root(decoding) };
        unsafe {
            assert_eq!(souther_read_array(root, ptr::null(), decoding), 1);
            assert_eq!(souther_read_array_length(root), Count(2));
            let second = souther_read_element(root, Count(1));
            let at = souther_path_at(souther_path_below(ptr::null(), literal("xs")), Count(1));
            let mut out = 0;
            assert_eq!(souther_read_int(second, at, decoding, &mut out), 0);
            assert_eq!(souther_read_array(second, at, decoding), 0);
        }
        assert_eq!(
            issues(decoding),
            vec![
                "/xs/1 type_mismatch actual=boolean expected=Int",
                "/xs/1 type_mismatch actual=boolean expected=an array",
            ]
        );
        souther_reset(mark);
    }

    /// Text arriving is canonicalized to NFC: か followed by a combining mark is read as が.
    #[test]
    fn text_is_read_canonicalized_to_nfc() {
        let mark = souther_mark();
        let decoding = begun("\"\u{304b}\u{3099}\"");
        let mut out = ptr::null_mut();
        let read = unsafe {
            souther_read_string(
                souther_decode_root(decoding),
                ptr::null(),
                decoding,
                &mut out,
            )
        };
        assert_eq!(read, 1);
        assert_eq!(said(out), "\u{304c}");
        assert_eq!(
            unsafe { souther_read_is(souther_decode_root(decoding), literal("\u{304c}")) },
            1
        );
        unsafe { souther_decode_abandon(decoding) };
        souther_reset(mark);
    }

    #[test]
    fn an_invariant_names_its_type_and_the_clause_where_it_has_a_name() {
        let mark = souther_mark();
        let decoding = begun("{}");
        unsafe {
            souther_read_invariant(
                ptr::null(),
                decoding,
                literal("shop"),
                literal("Money"),
                literal("notNegative"),
            );
            souther_read_invariant(
                souther_path_below(ptr::null(), literal("x")),
                decoding,
                literal("shop"),
                literal("Line"),
                ptr::null(),
            );
        }
        assert_eq!(
            issues(decoding),
            vec![
                " invariant_violation module=shop type=Money clause=notNegative",
                "/x invariant_violation module=shop type=Line",
            ]
        );
        souther_reset(mark);
    }

    #[test]
    fn bytes_that_are_not_a_document_say_where_and_read_as_nothing() {
        let mark = souther_mark();
        let decoding = begun("{\"a\":");
        unsafe {
            assert!(souther_decode_root(decoding).is_null());
            souther_decode_end(decoding, ptr::null());
            assert_eq!(souther_decoded_outcome(decoding), DECODED_MALFORMED);
            assert_eq!(souther_decoded_malformed_at(decoding), Count(5));
        }
        souther_reset(mark);
    }
}
