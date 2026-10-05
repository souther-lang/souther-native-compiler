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
//! What was found wrong is kept, all of it, as issues in Raoh's terms: a caller who fixes one field
//! and is then told about the next has been made to ask as many times as its document had mistakes.
//! An issue carries a code, the message key a resolver picks its wording by — the code itself where
//! Raoh gives no other — the JSON Pointer of where it was found, and what else it says about it as
//! Raoh's metadata: a JSON object whose entries are what they are, a number as a number and a list
//! as a list. Not a sentence: what a person reads is written against the key by whoever shows it.
//!
//! Everything a reading makes that outlives it — the reading itself, its issues, its paths and the
//! text in them — is taken from the arena, so the mark a caller took before decoding drops it with
//! everything else the call made. The document is the one thing on the heap, and the reading drops
//! it when it ends.

use crate::amount::Amount;
use crate::decimal::{Decimal, amount, decimal_of};
use crate::document::{Form, Node, parsed};
use crate::external::Form as Said;
use crate::temporal::{
    Date, DateTime, Instant, Time, date_of, date_time_of, instant_of, parse_date, parse_date_time,
    parse_instant, parse_time, time_of,
};
use crate::{Bool, Count, STRING_HOLDS, Text, Value, souther_alloc, string_of, text};
use souther_native_abi::{
    DECODED_ISSUES, DECODED_MALFORMED, DECODED_VALUE, META_TYPES, case_names_read,
};
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

/// One thing found wrong.
#[repr(C)]
pub struct Issue {
    code: *const u8,
    message_key: *const u8,
    path: *const u8,
    /// The metadata, as the JSON object it is written as.
    meta: *const u8,
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

/// A value of an issue's metadata as the type it is in Raoh's value model (`META_TYPES`), which
/// is what decides how it is written: the type each value is said to be is the one its declaration
/// gives, never one read off the JSON it would be written as, in which a `Decimal` of scale nought
/// and an `Int` are alike, and so are a date and a string.
#[derive(Debug)]
enum Meta {
    Int(i64),
    Decimal(Amount),
    String(Vec<u8>),
    Bool(bool),
    Date(Vec<u8>),
    Time(Vec<u8>),
    DateTime(Vec<u8>),
    Instant(Vec<u8>),
    List(Vec<Meta>),
}

impl Meta {
    /// The type's name in `META_TYPES`.
    fn type_name(&self) -> &'static str {
        match self {
            Meta::Int(_) => "int",
            Meta::Decimal(_) => "decimal",
            Meta::String(_) => "string",
            Meta::Bool(_) => "bool",
            Meta::Date(_) => "date",
            Meta::Time(_) => "time",
            Meta::DateTime(_) => "datetime",
            Meta::Instant(_) => "instant",
            Meta::List(_) => "list",
        }
    }

    /// As `META_TYPES` writes it: an object of one member, named for the type.
    fn written(&self) -> Said {
        let name = self.type_name();
        debug_assert!(META_TYPES.iter().any(|(type_name, _)| *type_name == name));
        let value = match self {
            Meta::Int(n) => Said::Number(*n),
            Meta::Decimal(amount) => Said::String(amount.scaled_text().into_bytes()),
            Meta::String(text)
            | Meta::Date(text)
            | Meta::Time(text)
            | Meta::DateTime(text)
            | Meta::Instant(text) => Said::String(text.clone()),
            Meta::Bool(truth) => Said::Bool(*truth),
            Meta::List(items) => Said::Array(items.iter().map(Meta::written).collect()),
        };
        Said::Object(vec![(name.as_bytes().to_vec(), value)])
    }

    /// An element of a list as the boundary wrote it, made the type `declared` names: one of
    /// `META_TYPES`, or `list<T>` of one. What generated code writes for an element of that type is
    /// the only form it is handed, so any other is a contract broken, and not a value to guess at.
    fn of(form: &Said, declared: &str) -> Meta {
        if let Some(element) = declared
            .strip_prefix("list<")
            .and_then(|it| it.strip_suffix('>'))
        {
            let Said::Array(items) = form else {
                panic!("a list<{element}> is written as an array");
            };
            return Meta::List(items.iter().map(|item| Meta::of(item, element)).collect());
        }
        match (declared, form) {
            ("int", Said::Number(n)) => Meta::Int(*n),
            ("decimal", Said::Amount(amount)) => Meta::Decimal(amount.clone()),
            ("string", Said::String(text)) => Meta::String(text.clone()),
            ("bool", Said::Bool(truth)) => Meta::Bool(*truth),
            ("date", Said::String(text)) => Meta::Date(text.clone()),
            ("time", Said::String(text)) => Meta::Time(text.clone()),
            ("datetime", Said::String(text)) => Meta::DateTime(text.clone()),
            ("instant", Said::String(text)) => Meta::Instant(text.clone()),
            (declared, form) => panic!("a {declared} is not written as {form:?}"),
        }
    }
}

/// The metadata of an issue as the JSON object it is written as: its entries in the order of their
/// names, one entry to a name, so one issue's metadata is written one way wherever it is read, and
/// each value as the type it is.
fn metadata(entries: Vec<(&str, Meta)>) -> String {
    let mut members: Vec<(Vec<u8>, Said)> = entries
        .into_iter()
        .map(|(name, meta)| (name.as_bytes().to_vec(), meta.written()))
        .collect();
    members.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(
        members.windows(2).all(|pair| pair[0].0 != pair[1].0),
        "an issue says one thing under a name"
    );
    let mut written = Vec::new();
    crate::external::write(&Said::Object(members), &mut written);
    String::from_utf8(written).expect("JSON written of text is text")
}

/// Text an issue says under a name.
fn words(text: &str) -> Meta {
    Meta::String(text.as_bytes().to_vec())
}

/// Records an issue found at `path`, under `key` where Raoh gives it a message key of its own and
/// under its code where it does not.
///
/// # Safety
/// `decoding` is one [`souther_decode_begin`] answered and still reading, and `path` is as
/// [`pointer`] says.
unsafe fn found(
    decoding: *mut Decoding,
    code: &str,
    key: Option<&str>,
    path: *const Path,
    meta: Vec<(&str, Meta)>,
) {
    let issue = held(Issue {
        code: string(code),
        message_key: string(key.unwrap_or(code)),
        path: string(&unsafe { pointer(path) }),
        meta: string(&metadata(meta)),
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

/// What a reading wanted where it found another kind of input, as `type_mismatch` names it in
/// `expected`: the kind of JSON Raoh's decoder of the type reads, which is what the JVM's
/// `jsonDecoder()` says, word for word. Each is the one statement of its word here.
mod wanted {
    pub(super) const STRING: &str = "string";
    pub(super) const INT: &str = "long";
    pub(super) const DECIMAL: &str = "number";
    pub(super) const BOOL: &str = "boolean";
    pub(super) const OBJECT: &str = "object";
    pub(super) const ARRAY: &str = "array";
}

/// Records that `node` is not the kind of input a reading takes at `path`: `required` where it is
/// `null`, which Raoh's decoders take as no value given and not as a value of the wrong kind, and
/// otherwise `type_mismatch`, with the kind found and the kind `wanted` (one of [`wanted`]).
unsafe fn mismatched(decoding: *mut Decoding, path: *const Path, node: &Node, wanted: &str) {
    if matches!(node, Node::Null) {
        unsafe { found(decoding, "required", None, path, Vec::new()) };
        return;
    }
    unsafe {
        found(
            decoding,
            "type_mismatch",
            None,
            path,
            vec![("actual", words(node.kind())), ("expected", words(wanted))],
        )
    };
}

/// Text arriving from outside, admitted where it arrives, which is at its string leaf: put in NFC
/// and within what a String holds (spec §string-canonical, §what-a-string-holds), or refused with
/// the issue code `invalid_format` at `path` — half of a surrogate pair as much as text with no
/// place is not the form a string's leaf takes.
unsafe fn admitted_text<'a>(
    written: &'a [u8],
    capacity: souther_text::Capacity,
    path: *const Path,
    decoding: *mut Decoding,
) -> Option<std::borrow::Cow<'a, str>> {
    match souther_text::admitted(written, capacity) {
        Ok(text) => Some(text),
        Err(souther_text::AdmissionRefusal::NotText | souther_text::AdmissionRefusal::NoPlace) => {
            unsafe { refused(decoding, path) };
            None
        }
    }
}

/// Begins reading `length` bytes at `bytes` as a document in the external form.
///
/// A count below nought is no document, and the reading comes to that: malformed at byte nought.
///
/// # Safety
/// `bytes` points at `length` bytes that may be read, where `length` is above nought, for as long
/// as this call runs: nothing after it reads them.
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
    // A count below nought is no document: the text stops being JSON before it starts.
    let parsing = match usize::try_from(length.0) {
        Ok(0) => parsed(&[], form),
        Ok(length) => parsed(unsafe { std::slice::from_raw_parts(bytes, length) }, form),
        Err(_) => Err(souther_json_syntax::Malformed { at: 0 }),
    };
    let (document, malformed_at) = match parsing {
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
) -> Bool {
    let node = unsafe { &*node };
    if node.length().is_some() {
        return Bool::TRUE;
    }
    unsafe { mismatched(decoding, path, node, wanted::ARRAY) };
    Bool::FALSE
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
) -> Bool {
    let node = unsafe { &*node };
    if node.is_object() {
        return Bool::TRUE;
    }
    unsafe { mismatched(decoding, path, node, wanted::OBJECT) };
    Bool::FALSE
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

/// How many members the object `node` holds, a key written twice counted twice.
///
/// # Safety
/// `node` is a place in a document being read, which [`souther_read_object`] said is an object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_members(node: *const Node) -> Count {
    Count(unsafe { (*node).members() }.len() as i64)
}

/// The key of the member of the object `node` at `index`, as the place of the document it is: a
/// string, which a map's key is read from as the key's own type.
///
/// # Safety
/// As [`souther_read_members`], and `index` is below what it answered.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_member_key(node: *const Node, index: Count) -> *const Node {
    let members = unsafe { (*node).members() };
    members[index.0 as usize].0.node()
}

/// What the member of the object `node` at `index` holds.
///
/// # Safety
/// As [`souther_read_member_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_member_value(node: *const Node, index: Count) -> *const Node {
    let members = unsafe { (*node).members() };
    &members[index.0 as usize].1
}

/// The place of the member of the object `node` at `index` below `path`, by its key as it was
/// written.
///
/// # Safety
/// `path` is null or one this answered, and the rest as [`souther_read_member_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_path_below_member(
    path: *const Path,
    node: *const Node,
    index: Count,
) -> *const Path {
    let members = unsafe { (*node).members() };
    let written = members[index.0 as usize].0.written();
    let step = std::str::from_utf8(written).expect("a key the parser read is text");
    held(Path {
        above: path,
        step: string(step),
    })
}

/// Records that the key at `path` is, once read as the map's key type, a key the map already has:
/// two spellings of one key, whose values would otherwise be one lost to the other with nothing
/// said. What the JVM's reader records (`duplicate_key`).
///
/// # Safety
/// As [`souther_read_missing`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_duplicate_key(path: *const Path, decoding: *mut Decoding) {
    unsafe { found(decoding, "duplicate_key", None, path, Vec::new()) };
}

/// Records that a field the declaration says every value has was not written, at `path`, which
/// is where it would have been: `required`, as Raoh's decoder of a field says it.
///
/// # Safety
/// As [`souther_read_object`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_missing(path: *const Path, decoding: *mut Decoding) {
    unsafe { found(decoding, "required", None, path, Vec::new()) };
}

/// Whether `node` is `null`, which is what absence is written as where there is no key to leave
/// out.
///
/// # Safety
/// `node` is a place in a document being read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_null(node: *const Node) -> Bool {
    Bool::from(matches!(unsafe { &*node }, Node::Null))
}

/// Writes what a scalar reader read through `out`, and answers whether it read one.
///
/// Every scalar reader writes its room whatever it answers: the value, or `none` where it read
/// nothing and recorded why. So generated code never holds what a stack slot happened to hold
/// before the call, whichever way the call went — the same holds of a reader of a declared type,
/// which writes its value or nothing whenever it answers — and the rule is kept here, once, rather
/// than by each reader remembering it.
unsafe fn answered<T>(out: *mut T, read: Option<T>, none: T) -> Bool {
    let there = read.is_some();
    unsafe { out.write(read.unwrap_or(none)) };
    Bool::from(there)
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
) -> Bool {
    let read = unsafe { int(&*node, path, decoding) };
    unsafe { answered(out, read, 0) }
}

unsafe fn int(node: &Node, path: *const Path, decoding: *mut Decoding) -> Option<i64> {
    let Node::Number(written) = node else {
        unsafe { mismatched(decoding, path, node, wanted::INT) };
        return None;
    };
    let (negative, digits) = match written.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, &written[..]),
    };
    // A point or an exponent means the document wrote an amount and not a whole number, whatever
    // amount it is.
    if !digits.iter().all(u8::is_ascii_digit) {
        unsafe { mismatched(decoding, path, node, wanted::INT) };
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
        // A whole number past what an Int holds is no number of the kind wanted, as Raoh's decoder
        // of a long says it, under a message key of its own.
        unsafe {
            found(
                decoding,
                "type_mismatch",
                Some("type_mismatch.numeric_range"),
                path,
                vec![("expected", words(wanted::INT))],
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
    out: *mut Bool,
) -> Bool {
    let read = match unsafe { &*node } {
        Node::Bool(truth) => Some(Bool::from(*truth)),
        other => {
            unsafe { mismatched(decoding, path, other, wanted::BOOL) };
            None
        }
    };
    unsafe { answered(out, read, Bool::FALSE) }
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
) -> Bool {
    let read = match unsafe { &*node } {
        Node::String(written) => {
            unsafe { admitted_text(written, STRING_HOLDS, path, decoding) }.map(|it| string_of(&it))
        }
        other => {
            unsafe { mismatched(decoding, path, other, wanted::STRING) };
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
) -> Bool {
    let read = match unsafe { &*node } {
        Node::Number(written) => {
            let read = Amount::of_json_number(written);
            if read.is_none() {
                unsafe {
                    found(
                        decoding,
                        "out_of_range",
                        None,
                        path,
                        vec![("actual", words("number")), ("expected", words("Decimal"))],
                    )
                };
            }
            read.as_ref().map(decimal_of)
        }
        other => {
            unsafe { mismatched(decoding, path, other, wanted::DECIMAL) };
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
        Node::String(written) => unsafe { admitted_text(written, STRING_HOLDS, path, decoding) },
        other => {
            unsafe { mismatched(decoding, path, other, wanted::STRING) };
            None
        }
    }
}

/// A place whose text is no value of a temporal type, or one a type held to the second would have
/// to round.
unsafe fn refused(decoding: *mut Decoding, path: *const Path) {
    unsafe { found(decoding, "invalid_format", None, path, Vec::new()) };
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
) -> Bool {
    let read = unsafe { temporal_text(&*node, path, decoding) }.and_then(|written| {
        let day = parse_date(&written);
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
) -> Bool {
    let read = unsafe { temporal_text(&*node, path, decoding) }.and_then(|written| {
        let second = parse_time(&written);
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
) -> Bool {
    let read = unsafe { temporal_text(&*node, path, decoding) }.and_then(|written| {
        let second = parse_date_time(&written);
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
) -> Bool {
    let read = unsafe { temporal_text(&*node, path, decoding) }.and_then(|written| {
        let moment = parse_instant(&written);
        if moment.is_none() {
            unsafe { refused(decoding, path) };
        }
        moment.map(|(second, nano)| instant_of(second, nano))
    });
    unsafe { answered(out, read, ptr::null_mut()) }
}

/// [`souther_read_case`] against `capacity` rather than always [`STRING_HOLDS`]: a test that
/// cannot build a case name past the real bound asks this of a small one instead, the way a test
/// of [`souther_text::admitted`] itself does.
unsafe fn read_case(
    node: &Node,
    capacity: souther_text::Capacity,
    path: *const Path,
    decoding: *mut Decoding,
) -> Bool {
    let Node::String(written) = node else {
        // A case is named by text, so what is not text is refused as Raoh's string decoder refuses
        // it, which is what its `discriminate` and its `oneOf` over strings read a name with.
        match node {
            Node::Null => unsafe { found(decoding, "required", None, path, Vec::new()) },
            other => unsafe { mismatched(decoding, path, other, wanted::STRING) },
        }
        return Bool::FALSE;
    };
    Bool::from(unsafe { admitted_text(written, capacity, path, decoding) }.is_some())
}

/// Whether `node` is text naming a case, having recorded that it is not where it is not, or that
/// its canonical value has no place a case spelling could hold. Admission is settled here, once:
/// [`souther_read_is`] and [`souther_read_not_a_case`] read `node`'s text only after this has
/// answered 1 for it, and rely on that admission already having succeeded. Which case it names is
/// asked of it next ([`souther_read_is`]).
///
/// # Safety
/// As [`souther_read_object`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_case(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
) -> Bool {
    unsafe { read_case(&*node, STRING_HOLDS, path, decoding) }
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
        // A tag not there is what Raoh's `discriminate` reads it as: what its string decoder gives
        // for a value that is absent.
        unsafe { found(decoding, "required", None, path, Vec::new()) };
        return ptr::null();
    };
    if unsafe { souther_read_case(tag, path, decoding) } == Bool::FALSE {
        return ptr::null();
    }
    tag
}

/// Whether the text `node` writes is `name`, once it is canonicalized as any text arriving is.
///
/// # Safety
/// `node` is a place in a document being read that holds text; `name` is a string of the
/// runtime's layout. `node` is one [`souther_read_case`] has already answered 1 for: this takes no
/// `path` or `decoding` to report a refusal with, because admission was already settled there, and
/// asking again of the same text cannot answer differently.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_is(node: *const Node, name: *const Text) -> Bool {
    let Node::String(written) = (unsafe { &*node }) else {
        panic!("souther_read_case admitted this node, which holds a string and no other form");
    };
    let admitted = souther_text::admitted(written, STRING_HOLDS)
        .expect("souther_read_case admitted this text");
    Bool::from(admitted.as_bytes() == unsafe { text(&name) }.as_bytes())
}

/// Records that the text `node` writes names no case there is, at `path`: Raoh's `not_allowed` with
/// the name written. What an object of this generation built before [`souther_read_not_one_of`]
/// calls, which hands over no names to say were allowed.
///
/// # Safety
/// As [`souther_read_object`], and `node` is one [`souther_read_case`] has already answered 1 for.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_not_a_case(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
) {
    let written = unsafe { name_written(node) };
    unsafe {
        found(
            decoding,
            "not_allowed",
            None,
            path,
            vec![("actual", words(&written))],
        )
    };
}

/// Records that the text `node` writes is the name of none of an enumeration's cases, at `path`:
/// Raoh's `not_allowed`, with the names allowed and the name written, as its `oneOf` over strings
/// reports it, since that constraint states the same rule (spec §sum-discrimination).
///
/// # Safety
/// As [`souther_read_not_a_case`], and `names` is a string of the runtime's layout holding what
/// `case_names_written` wrote.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_not_one_of(
    node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    names: *const Text,
) {
    let written = unsafe { name_written(node) };
    unsafe {
        found(
            decoding,
            "not_allowed",
            None,
            path,
            vec![("allowed", allowed(names)), ("actual", words(&written))],
        )
    };
}

/// What was written where a case's name is, as the text it is admitted as: every string holds text
/// in NFC and within what a String holds, which `souther_read_case` already established for it.
///
/// # Safety
/// `node` is one [`souther_read_case`] has already answered 1 for.
unsafe fn name_written<'a>(node: *const Node) -> std::borrow::Cow<'a, str> {
    let Node::String(written) = (unsafe { &*node }) else {
        panic!("souther_read_case admitted this node, which holds a string and no other form");
    };
    souther_text::admitted(written, STRING_HOLDS).expect("souther_read_case admitted this text")
}

/// Records that the tag an object names its case with, at `path`, names none of a sum's cases:
/// Raoh's `not_allowed`, with the names allowed and nothing else, as its `discriminate` reports it.
///
/// # Safety
/// As [`souther_read_not_one_of`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_no_such_tag(
    _node: *const Node,
    path: *const Path,
    decoding: *mut Decoding,
    names: *const Text,
) {
    unsafe {
        found(
            decoding,
            "not_allowed",
            None,
            path,
            vec![("allowed", allowed(names))],
        )
    };
}

/// The names a value was allowed to be, as an issue lists them.
///
/// # Safety
/// `names` is a string of the runtime's layout holding what `case_names_written` wrote.
unsafe fn allowed(names: *const Text) -> Meta {
    Meta::List(
        case_names_read(unsafe { text(&names) }.as_str())
            .map(words)
            .collect(),
    )
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
    let mut meta = vec![("module", words(module)), ("type", words(name))];
    if !clause.is_null() {
        meta.push(("clause", words(unsafe { text(&clause).as_str() })));
    }
    unsafe { found(decoding, "invariant_violation", None, path, meta) };
}

// What a clause stated as a standard constraint reports where the value breaks it: the code, the
// message key and the metadata Raoh's own constraint reports, so a host reading these reads what
// the JVM's decoder answers for the same model. Each answers whether the value meets it, having
// recorded that it does not where it does not. Which constraint a clause is was the checker's; this
// is only what Raoh calls each.

/// Where `holds` is false, records `code` under `key` at `path` with `meta`; answers `holds`.
///
/// # Safety
/// As [`found`].
unsafe fn meets(
    holds: bool,
    decoding: *mut Decoding,
    path: *const Path,
    code: &str,
    key: Option<&str>,
    meta: impl FnOnce() -> Vec<(&'static str, Meta)>,
) -> Bool {
    if !holds {
        unsafe { found(decoding, code, key, path, meta()) };
    }
    Bool::from(holds)
}

/// How many characters `value` holds, counted as `String.length` counts them.
unsafe fn characters(value: *const Text) -> i64 {
    unsafe { crate::souther_string_code_points(value) }
}

/// How many elements the list `value` holds.
unsafe fn elements_of(value: *const crate::List) -> i64 {
    unsafe {
        value
            .cast::<u8>()
            .offset(souther_native_abi::LIST_LENGTH as isize)
            .cast::<i64>()
            .read()
    }
}

/// `StringDecoder.minLength(n)`: `too_short` with `min` and `actual`.
///
/// # Safety
/// As [`souther_read_invariant`]; `value` is a string of the runtime's layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_min_length(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const Text,
    n: i64,
) -> Bool {
    let actual = unsafe { characters(value) };
    unsafe {
        meets(actual >= n, decoding, path, "too_short", None, || {
            vec![("min", Meta::Int(n)), ("actual", Meta::Int(actual))]
        })
    }
}

/// `StringDecoder.maxLength(n)`: `too_long` with `max` and `actual`.
///
/// # Safety
/// As [`souther_read_min_length`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_max_length(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const Text,
    n: i64,
) -> Bool {
    let actual = unsafe { characters(value) };
    unsafe {
        meets(actual <= n, decoding, path, "too_long", None, || {
            vec![("max", Meta::Int(n)), ("actual", Meta::Int(actual))]
        })
    }
}

/// `StringDecoder.fixedLength(n)`: `invalid_length` with `expected` and `actual`.
///
/// # Safety
/// As [`souther_read_min_length`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_fixed_length(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const Text,
    n: i64,
) -> Bool {
    let actual = unsafe { characters(value) };
    unsafe {
        meets(actual == n, decoding, path, "invalid_length", None, || {
            vec![("expected", Meta::Int(n)), ("actual", Meta::Int(actual))]
        })
    }
}

/// A pattern the whole of `value` has to match: `invalid_format` with the `pattern` it was held
/// to, which is the text the author's call was given and not the machine that ran it.
///
/// # Safety
/// As [`souther_read_min_length`]; `held` is a pattern the object carries, as
/// [`crate::souther_string_matches`] takes it, and `written` a string of the runtime's layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_pattern(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const Text,
    held: *const crate::kernels::HeldPattern,
    written: *const Text,
) -> Bool {
    let holds = unsafe { crate::souther_string_matches(held, value) } != Bool::FALSE;
    unsafe {
        meets(holds, decoding, path, "invalid_format", None, || {
            vec![("pattern", words(text(&written).as_str()))]
        })
    }
}

/// `LongDecoder.min(n)`: `out_of_range` under `out_of_range.minimum`, with `min` and `actual`.
///
/// # Safety
/// As [`souther_read_invariant`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_int_min(
    path: *const Path,
    decoding: *mut Decoding,
    value: i64,
    n: i64,
) -> Bool {
    unsafe {
        meets(
            value >= n,
            decoding,
            path,
            "out_of_range",
            Some("out_of_range.minimum"),
            || vec![("min", Meta::Int(n)), ("actual", Meta::Int(value))],
        )
    }
}

/// `LongDecoder.max(n)`: `out_of_range` under `out_of_range.maximum`, with `max` and `actual`.
///
/// # Safety
/// As [`souther_read_invariant`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_int_max(
    path: *const Path,
    decoding: *mut Decoding,
    value: i64,
    n: i64,
) -> Bool {
    unsafe {
        meets(
            value <= n,
            decoding,
            path,
            "out_of_range",
            Some("out_of_range.maximum"),
            || vec![("max", Meta::Int(n)), ("actual", Meta::Int(value))],
        )
    }
}

/// `LongDecoder.positive()`: `out_of_range` under `out_of_range.positive`, with `min` one.
///
/// # Safety
/// As [`souther_read_invariant`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_int_positive(
    path: *const Path,
    decoding: *mut Decoding,
    value: i64,
) -> Bool {
    unsafe {
        meets(
            value > 0,
            decoding,
            path,
            "out_of_range",
            Some("out_of_range.positive"),
            || vec![("min", Meta::Int(1)), ("actual", Meta::Int(value))],
        )
    }
}

/// `LongDecoder.nonNegative()`: `out_of_range` under `out_of_range.non_negative`, with `min`
/// nought.
///
/// # Safety
/// As [`souther_read_invariant`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_int_non_negative(
    path: *const Path,
    decoding: *mut Decoding,
    value: i64,
) -> Bool {
    unsafe {
        meets(
            value >= 0,
            decoding,
            path,
            "out_of_range",
            Some("out_of_range.non_negative"),
            || vec![("min", Meta::Int(0)), ("actual", Meta::Int(value))],
        )
    }
}

/// A `Decimal` as metadata holds it: the value, scale and all, which [`metadata`] writes at its
/// scale.
unsafe fn amount_said(at: *const Decimal) -> Meta {
    Meta::Decimal(unsafe { amount(at) })
}

/// The `Decimal` nought a bound of a `Decimal` is stated against: a `Decimal` and not the `Int` 0,
/// as Raoh's `DecimalDecoder` holds `BigDecimal.ZERO`, so a host's issue says the same type.
fn decimal_zero() -> Meta {
    Meta::Decimal(Amount::of_int(0))
}

/// `DecimalDecoder.min(n)`: `out_of_range` under `out_of_range.minimum`, with `min` and `actual`,
/// compared by amount whatever the scales.
///
/// # Safety
/// As [`souther_read_invariant`]; `value` and `n` are `Decimal`s of the runtime's.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_decimal_min(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const Decimal,
    n: *const Decimal,
) -> Bool {
    let holds = unsafe { amount(value).compare(&amount(n)) }.is_ge();
    unsafe {
        meets(
            holds,
            decoding,
            path,
            "out_of_range",
            Some("out_of_range.minimum"),
            || vec![("min", amount_said(n)), ("actual", amount_said(value))],
        )
    }
}

/// `DecimalDecoder.max(n)`: `out_of_range` under `out_of_range.maximum`, with `max` and `actual`.
///
/// # Safety
/// As [`souther_read_decimal_min`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_decimal_max(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const Decimal,
    n: *const Decimal,
) -> Bool {
    let holds = unsafe { amount(value).compare(&amount(n)) }.is_le();
    unsafe {
        meets(
            holds,
            decoding,
            path,
            "out_of_range",
            Some("out_of_range.maximum"),
            || vec![("max", amount_said(n)), ("actual", amount_said(value))],
        )
    }
}

/// Where a `Decimal` stands against nought.
unsafe fn sign_of(value: *const Decimal) -> std::cmp::Ordering {
    unsafe { amount(value) }.compare(&crate::amount::Amount::of_int(0))
}

/// `DecimalDecoder.positive()`: `out_of_range` under `out_of_range.positive`, with `min` nought —
/// the bound the value has to be above, as Raoh's own says it, and not a least value.
///
/// # Safety
/// As [`souther_read_decimal_min`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_decimal_positive(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const Decimal,
) -> Bool {
    let holds = unsafe { sign_of(value) }.is_gt();
    unsafe {
        meets(
            holds,
            decoding,
            path,
            "out_of_range",
            Some("out_of_range.positive"),
            || vec![("min", decimal_zero()), ("actual", amount_said(value))],
        )
    }
}

/// `DecimalDecoder.nonNegative()`: `out_of_range` under `out_of_range.non_negative`, with `min`
/// nought.
///
/// # Safety
/// As [`souther_read_decimal_min`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_decimal_non_negative(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const Decimal,
) -> Bool {
    let holds = unsafe { sign_of(value) }.is_ge();
    unsafe {
        meets(
            holds,
            decoding,
            path,
            "out_of_range",
            Some("out_of_range.non_negative"),
            || vec![("min", decimal_zero()), ("actual", amount_said(value))],
        )
    }
}

/// How a collection's size is refused, the same for a list's elements and a map's entries: Raoh's
/// `nonempty` is `too_small` under `too_small.nonempty` with `min` one and `actual` nought, a least
/// size `too_small` with `min`, a most `too_big` with `max`, an exact one `invalid_size` with
/// `expected`, each with the `actual` size.
unsafe fn sized(decoding: *mut Decoding, path: *const Path, actual: i64, bound: Bound) -> Bool {
    let (holds, code, key, name, n) = match bound {
        Bound::NonEmpty => (
            actual > 0,
            "too_small",
            Some("too_small.nonempty"),
            "min",
            1,
        ),
        Bound::AtLeast(n) => (actual >= n, "too_small", None, "min", n),
        Bound::AtMost(n) => (actual <= n, "too_big", None, "max", n),
        Bound::Exactly(n) => (actual == n, "invalid_size", None, "expected", n),
    };
    unsafe {
        meets(holds, decoding, path, code, key, || {
            vec![(name, Meta::Int(n)), ("actual", Meta::Int(actual))]
        })
    }
}

/// Which size a collection is held to.
enum Bound {
    NonEmpty,
    AtLeast(i64),
    AtMost(i64),
    Exactly(i64),
}

/// `ListDecoder.nonempty()`.
///
/// # Safety
/// As [`souther_read_invariant`]; `value` is a list of the runtime's layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_list_non_empty(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const crate::List,
) -> Bool {
    unsafe { sized(decoding, path, elements_of(value), Bound::NonEmpty) }
}

/// `ListDecoder.minSize(n)`.
///
/// # Safety
/// As [`souther_read_list_non_empty`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_list_min_size(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const crate::List,
    n: i64,
) -> Bool {
    unsafe { sized(decoding, path, elements_of(value), Bound::AtLeast(n)) }
}

/// `ListDecoder.maxSize(n)`.
///
/// # Safety
/// As [`souther_read_list_non_empty`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_list_max_size(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const crate::List,
    n: i64,
) -> Bool {
    unsafe { sized(decoding, path, elements_of(value), Bound::AtMost(n)) }
}

/// `ListDecoder.fixedSize(n)`.
///
/// # Safety
/// As [`souther_read_list_non_empty`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_list_fixed_size(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const crate::List,
    n: i64,
) -> Bool {
    unsafe { sized(decoding, path, elements_of(value), Bound::Exactly(n)) }
}

/// `ListDecoder.unique()` refused: `duplicate_element` with the `duplicates`, the elements the list
/// holds more than once, which generated code wrote into `duplicates` as a boundary writes them,
/// and each the type `element` names (`META_TYPES`, or `list<T>` of one), which the compiler gave
/// from the list's declaration. Which elements those are is [`crate::souther_list_duplicates`]'s
/// answer.
///
/// # Safety
/// As [`souther_read_invariant`]; `duplicates` is a form the caller owns, and owns no longer once
/// this returns, and `element` is a string of the runtime's layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_duplicates(
    path: *const Path,
    decoding: *mut Decoding,
    duplicates: *mut Said,
    element: *const Text,
) {
    let duplicates = *unsafe { Box::from_raw(duplicates) };
    let element = unsafe { text(&element) };
    unsafe {
        found(
            decoding,
            "duplicate_element",
            None,
            path,
            vec![(
                "duplicates",
                Meta::of(&duplicates, &format!("list<{}>", element.as_str())),
            )],
        )
    };
}

/// The emptiness of a map, as the JVM's decoder refuses it: what `RecordDecoder.nonempty()`
/// reports.
///
/// # Safety
/// As [`souther_read_invariant`]; `value` is a map of the runtime's.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_map_non_empty(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const crate::Map,
) -> Bool {
    unsafe {
        sized(
            decoding,
            path,
            crate::souther_map_size(value),
            Bound::NonEmpty,
        )
    }
}

/// A least count of entries.
///
/// # Safety
/// As [`souther_read_map_non_empty`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_map_min_size(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const crate::Map,
    n: i64,
) -> Bool {
    unsafe {
        sized(
            decoding,
            path,
            crate::souther_map_size(value),
            Bound::AtLeast(n),
        )
    }
}

/// A most count of entries.
///
/// # Safety
/// As [`souther_read_map_non_empty`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_read_map_max_size(
    path: *const Path,
    decoding: *mut Decoding,
    value: *const crate::Map,
    n: i64,
) -> Bool {
    unsafe {
        sized(
            decoding,
            path,
            crate::souther_map_size(value),
            Bound::AtMost(n),
        )
    }
}

/// What a reading came to: [`DECODED_VALUE`], [`DECODED_ISSUES`] or [`DECODED_MALFORMED`].
///
/// # Safety
/// `decoded` is a reading a decoder answered, and the scope it was made in is still open.
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

/// The issue at `at`, counting from nought in the order they were found, or null where there is
/// none there: a place is a number a host can have wrong, and it is answered as that.
///
/// # Safety
/// As [`souther_decoded_outcome`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decoded_issue(
    decoded: *const Decoding,
    at: Count,
) -> *const Issue {
    let Count(at) = at;
    let decoded = unsafe { &*decoded };
    if !(0..decoded.count).contains(&at) {
        return ptr::null();
    }
    unsafe { *decoded.issues.add(at as usize) }
}

/// The issue's code, as a string of the runtime's layout.
///
/// # Safety
/// `issue` is one [`souther_decoded_issue`] answered, and the scope it was made in is still open.
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

/// The key a resolver picks the issue's wording by: Raoh's message key where it gives the issue one
/// of its own (`out_of_range.minimum`), and its code where it gives none. Always one or the other,
/// as Raoh's own `Issue` holds it.
///
/// # Safety
/// As [`souther_issue_code`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_issue_message_key(issue: *const Issue) -> *const Text {
    unsafe { (*issue).message_key.cast() }
}

/// What else the issue says, as the JSON object Raoh's metadata is: its entries in the order of
/// their names, a number written as a number and a list as a list. `{}` where it says nothing more.
///
/// # Safety
/// As [`souther_issue_code`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_issue_meta(issue: *const Issue) -> *const Text {
    unsafe { (*issue).meta.cast() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{souther_scope_close, souther_scope_open, souther_string_of_utf8};

    fn said(at: *const Text) -> String {
        String::from_utf8(unsafe { text(&at).as_bytes() }.to_vec()).unwrap()
    }

    fn literal(value: &str) -> *mut Text {
        let mut out = ptr::null_mut();
        let admitted =
            unsafe { souther_string_of_utf8(value.as_ptr(), Count(value.len() as i64), &mut out) };
        assert_eq!(admitted, Bool::TRUE, "test text has a place");
        out
    }

    fn begun(document: &str) -> *mut Decoding {
        unsafe { souther_decode_begin(document.as_ptr(), Count(document.len() as i64)) }
    }

    /// Every issue a reading found, as `path code meta`, with ` key=…` after the code where the
    /// message key is not the code.
    fn issues(decoding: *mut Decoding) -> Vec<String> {
        unsafe {
            souther_decode_end(decoding, ptr::null());
            (0..souther_decoded_issue_count(decoding).0)
                .map(|at| {
                    let issue = souther_decoded_issue(decoding, Count(at));
                    let code = said(souther_issue_code(issue));
                    let key = said(souther_issue_message_key(issue));
                    let keyed = if key == code {
                        String::new()
                    } else {
                        format!(" key={key}")
                    };
                    format!(
                        "{} {code}{keyed} {}",
                        said(souther_issue_path(issue)),
                        said(souther_issue_meta(issue))
                    )
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
        if read == Bool::TRUE {
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
        let scope = souther_scope_open();
        let decoding = begun("{}");
        let root = unsafe { souther_decode_root(decoding) };
        let mut int = -7;
        let mut truth = Bool(7);
        let mut text = literal("before");
        unsafe {
            assert_eq!(
                souther_read_int(root, ptr::null(), decoding, &mut int),
                Bool::FALSE
            );
            assert_eq!(
                souther_read_bool(root, ptr::null(), decoding, &mut truth),
                Bool::FALSE
            );
            assert_eq!(
                souther_read_string(root, ptr::null(), decoding, &mut text),
                Bool::FALSE
            );
            souther_decode_abandon(decoding);
        }
        assert_eq!((int, truth, text), (0, Bool::FALSE, ptr::null_mut()));
        souther_scope_close(scope);
    }

    #[test]
    fn an_int_is_a_whole_number_within_sixty_four_bits() {
        let scope = souther_scope_open();
        assert_eq!(int("42"), Ok(42));
        assert_eq!(int("-0"), Ok(0));
        assert_eq!(int("9223372036854775807"), Ok(i64::MAX));
        assert_eq!(int("-9223372036854775808"), Ok(i64::MIN));
        assert_eq!(
            int("9223372036854775808"),
            Err(vec![
                r#" type_mismatch key=type_mismatch.numeric_range {"expected":{"string":"long"}}"#
                    .to_string()
            ])
        );
        assert_eq!(
            int("-99999999999999999999999"),
            Err(vec![
                r#" type_mismatch key=type_mismatch.numeric_range {"expected":{"string":"long"}}"#
                    .to_string()
            ])
        );
        assert_eq!(
            int("1.0"),
            Err(vec![
                r#" type_mismatch {"actual":{"string":"number"},"expected":{"string":"long"}}"#
                    .to_string()
            ])
        );
        assert_eq!(
            int("1e0"),
            Err(vec![
                r#" type_mismatch {"actual":{"string":"number"},"expected":{"string":"long"}}"#
                    .to_string()
            ])
        );
        assert_eq!(
            int("\"1\""),
            Err(vec![
                r#" type_mismatch {"actual":{"string":"string"},"expected":{"string":"long"}}"#
                    .to_string()
            ])
        );
        souther_scope_close(scope);
    }

    /// A key holding `/` or `~` is one step of the pointer and not two (RFC 6901).
    #[test]
    fn a_path_is_a_json_pointer_with_its_steps_escaped() {
        let scope = souther_scope_open();
        let decoding = begun("{}");
        let path = unsafe {
            let a = souther_path_below(ptr::null(), literal("a/b"));
            let b = souther_path_below(a, literal("~c"));
            souther_path_below(b, literal("0"))
        };
        unsafe { souther_read_missing(path, decoding) };
        assert_eq!(issues(decoding), vec![r#"/a~1b/~0c/0 required {}"#]);
        souther_scope_close(scope);
    }

    /// An element's place is its index below the array's, and a place that is not an array is
    /// recorded as one.
    #[test]
    fn an_array_is_read_element_by_element_at_its_index() {
        let scope = souther_scope_open();
        let decoding = begun("[1, true]");
        let root = unsafe { souther_decode_root(decoding) };
        unsafe {
            assert_eq!(souther_read_array(root, ptr::null(), decoding), Bool::TRUE);
            assert_eq!(souther_read_array_length(root), Count(2));
            let second = souther_read_element(root, Count(1));
            let at = souther_path_at(souther_path_below(ptr::null(), literal("xs")), Count(1));
            let mut out = 0;
            assert_eq!(
                souther_read_int(second, at, decoding, &mut out),
                Bool::FALSE
            );
            assert_eq!(souther_read_array(second, at, decoding), Bool::FALSE);
        }
        assert_eq!(
            issues(decoding),
            vec![
                r#"/xs/1 type_mismatch {"actual":{"string":"boolean"},"expected":{"string":"long"}}"#,
                r#"/xs/1 type_mismatch {"actual":{"string":"boolean"},"expected":{"string":"array"}}"#,
            ]
        );
        souther_scope_close(scope);
    }

    /// Text arriving is canonicalized to NFC: か followed by a combining mark is read as が.
    #[test]
    fn text_is_read_canonicalized_to_nfc() {
        let scope = souther_scope_open();
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
        assert_eq!(read, Bool::TRUE);
        assert_eq!(said(out), "\u{304c}");
        assert_eq!(
            unsafe { souther_read_is(souther_decode_root(decoding), literal("\u{304c}")) },
            Bool::TRUE
        );
        unsafe { souther_decode_abandon(decoding) };
        souther_scope_close(scope);
    }

    /// A string leaf whose canonical value has no place at what a `String` holds is refused with
    /// `invalid_format`, the same code half of a surrogate pair is refused with — never a silent
    /// `false` that a case-reading caller (`souther_read_is`) would go on to record as
    /// `not_allowed`, which conflated "not this case" with "not a case at all"
    /// (souther-native-compiler#109). `admitted_text` is what every string-leaf reader
    /// (`souther_read_string`, `souther_read_case`, `temporal_text`) routes through, so pinning it
    /// here pins all of them; a document string long enough to exceed the real
    /// `STRING_HOLDS` (2^28 - 1 code points) is not something a test can build, so this asks the
    /// same gate of a small capacity instead, exactly as the language's own test of the bound does
    /// (spec §what-a-string-holds).
    #[test]
    fn text_with_no_place_is_invalid_format_and_not_a_silent_refusal() {
        let scope = souther_scope_open();
        let decoding = begun("{}");
        let refused = unsafe {
            admitted_text(
                b"abcd",
                souther_text::Capacity::of_code_points(3),
                ptr::null(),
                decoding,
            )
        };
        assert!(refused.is_none());
        let found = issues(decoding);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("invalid_format"), "{found:?}");
        souther_scope_close(scope);
    }

    /// The regression `souther-native-compiler#109` actually was: a case name whose canonical
    /// value has no place must be refused at [`souther_read_case`] — the gate every case-reading
    /// door is a protocol invariant of — as `invalid_format`, and `not_allowed` (the code for "text
    /// that names none of the cases there are") must never be recorded for it. The test above pins
    /// the shared `admitted_text` helper directly; this one pins the actual gate function, through
    /// [`read_case`]'s test-only capacity parameter, so that a future change unwiring the capacity
    /// check from `souther_read_case` itself — leaving `admitted_text` correct but unreachable from
    /// this door — still fails a test.
    #[test]
    fn an_oversized_case_name_is_invalid_format_at_the_gate_and_never_not_allowed() {
        let scope = souther_scope_open();
        let decoding = begun("{}");
        let oversized = Node::String(Box::from(*b"abcd"));
        let gated = unsafe {
            read_case(
                &oversized,
                souther_text::Capacity::of_code_points(3),
                ptr::null(),
                decoding,
            )
        };
        assert_eq!(
            gated,
            Bool::FALSE,
            "a case name past capacity is not admitted"
        );
        let found = issues(decoding);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("invalid_format"), "{found:?}");
        assert!(
            !found.iter().any(|it| it.contains("not_allowed")),
            "the gate must not fall through to \"not a case\": {found:?}"
        );
        souther_scope_close(scope);
    }

    #[test]
    fn an_invariant_names_its_type_and_the_clause_where_it_has_a_name() {
        let scope = souther_scope_open();
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
                r#" invariant_violation {"clause":{"string":"notNegative"},"module":{"string":"shop"},"type":{"string":"Money"}}"#,
                r#"/x invariant_violation {"module":{"string":"shop"},"type":{"string":"Line"}}"#,
            ]
        );
        souther_scope_close(scope);
    }

    fn decimal(unscaled: &str, scale: i64) -> *mut Decimal {
        let mut out = std::ptr::null_mut();
        let made = unsafe {
            crate::souther_decimal_of_parts(
                unscaled.as_ptr(),
                Count(unscaled.len() as i64),
                scale,
                &mut out,
            )
        };
        assert_eq!(made, Bool::TRUE, "{unscaled} at {scale} names a Decimal");
        out
    }

    fn ints(values: &[i64]) -> *mut crate::List {
        crate::kernels::list_of(values, |it| *it)
    }

    /// Each constraint answers whether the value meets it, and where it does not records what
    /// Raoh's own constraint reports: its code, its message key where it has one of its own, and
    /// its metadata with numbers as numbers — a `Decimal` at its scale, as Raoh's own holds it.
    #[test]
    fn a_constraint_a_value_breaks_is_reported_as_raohs() {
        let scope = souther_scope_open();
        let decoding = begun("{}");
        let at = |step: &str| unsafe { souther_path_below(ptr::null(), literal(step)) };
        let held = unsafe {
            [
                souther_read_min_length(at("a"), decoding, literal("\u{304c}b"), 3),
                souther_read_min_length(at("a"), decoding, literal("abc"), 3),
                souther_read_max_length(at("b"), decoding, literal("abcd"), 3),
                souther_read_fixed_length(at("c"), decoding, literal("ab"), 3),
                souther_read_int_min(at("d"), decoding, 2, 3),
                souther_read_int_max(at("e"), decoding, 4, 3),
                souther_read_int_positive(at("f"), decoding, 0),
                souther_read_int_non_negative(at("g"), decoding, -1),
                souther_read_int_non_negative(at("g"), decoding, 0),
                souther_read_decimal_min(at("h"), decoding, decimal("150", 2), decimal("200", 2)),
                souther_read_decimal_max(at("i"), decoding, decimal("250", 2), decimal("2", 0)),
                souther_read_decimal_positive(at("j"), decoding, decimal("0", 2)),
                souther_read_decimal_non_negative(at("k"), decoding, decimal("-1", 1)),
                souther_read_list_non_empty(at("l"), decoding, ints(&[])),
                souther_read_list_min_size(at("m"), decoding, ints(&[1]), 2),
                souther_read_list_max_size(at("n"), decoding, ints(&[1, 2, 3]), 2),
                souther_read_list_fixed_size(at("o"), decoding, ints(&[1]), 2),
                souther_read_list_fixed_size(at("o"), decoding, ints(&[1, 2]), 2),
            ]
        };
        assert_eq!(
            held.map(Bool::is),
            [
                false, true, false, false, false, false, false, false, true, false, false, false,
                false, false, false, false, false, true
            ]
        );
        assert_eq!(
            issues(decoding),
            vec![
                r#"/a too_short {"actual":{"int":2},"min":{"int":3}}"#,
                r#"/b too_long {"actual":{"int":4},"max":{"int":3}}"#,
                r#"/c invalid_length {"actual":{"int":2},"expected":{"int":3}}"#,
                r#"/d out_of_range key=out_of_range.minimum {"actual":{"int":2},"min":{"int":3}}"#,
                r#"/e out_of_range key=out_of_range.maximum {"actual":{"int":4},"max":{"int":3}}"#,
                r#"/f out_of_range key=out_of_range.positive {"actual":{"int":0},"min":{"int":1}}"#,
                r#"/g out_of_range key=out_of_range.non_negative {"actual":{"int":-1},"min":{"int":0}}"#,
                r#"/h out_of_range key=out_of_range.minimum {"actual":{"decimal":"1.50"},"min":{"decimal":"2.00"}}"#,
                r#"/i out_of_range key=out_of_range.maximum {"actual":{"decimal":"2.50"},"max":{"decimal":"2"}}"#,
                r#"/j out_of_range key=out_of_range.positive {"actual":{"decimal":"0.00"},"min":{"decimal":"0"}}"#,
                r#"/k out_of_range key=out_of_range.non_negative {"actual":{"decimal":"-0.1"},"min":{"decimal":"0"}}"#,
                r#"/l too_small key=too_small.nonempty {"actual":{"int":0},"min":{"int":1}}"#,
                r#"/m too_small {"actual":{"int":1},"min":{"int":2}}"#,
                r#"/n too_big {"actual":{"int":3},"max":{"int":2}}"#,
                r#"/o invalid_size {"actual":{"int":1},"expected":{"int":2}}"#,
            ]
        );
        souther_scope_close(scope);
    }

    /// The elements a list repeats are each answered once, in the order their repetition was
    /// found, and reported as the form they are written in.
    #[test]
    fn a_list_repeating_elements_reports_each_once() {
        let scope = souther_scope_open();
        extern "C" fn hash(value: i64) -> crate::Hash {
            crate::Hash(value)
        }
        extern "C" fn equal(a: i64, b: i64) -> Bool {
            Bool::from(a == b)
        }
        let repeated =
            unsafe { crate::souther_list_duplicates(ints(&[3, 1, 3, 2, 1, 3]), hash, equal) };
        assert_eq!(unsafe { elements_of(repeated) }, 2);
        let none = unsafe { crate::souther_list_duplicates(ints(&[1, 2]), hash, equal) };
        assert_eq!(unsafe { elements_of(none) }, 0);

        let decoding = begun("{}");
        let form = crate::external::handed(Said::Array(vec![Said::Number(3), Said::Number(1)]));
        unsafe { souther_read_duplicates(ptr::null(), decoding, form, literal("int")) };
        assert_eq!(
            issues(decoding),
            vec![r#" duplicate_element {"duplicates":{"list":[{"int":3},{"int":1}]}}"#]
        );
        souther_scope_close(scope);
    }

    /// The elements `duplicates` lists are each the type the declaration gives them, whatever
    /// they are written as: a date is not text, and a `Decimal` of scale nought is not an `Int`.
    #[test]
    fn the_elements_a_list_repeats_are_the_type_their_declaration_gives() {
        let scope = souther_scope_open();
        let decoding = begun("{}");
        let reported = |elements: Vec<Said>, element: &str| unsafe {
            souther_read_duplicates(
                ptr::null(),
                decoding,
                crate::external::handed(Said::Array(elements)),
                literal(element),
            )
        };
        reported(vec![Said::String(b"2026-01-31".to_vec())], "date");
        reported(
            vec![Said::Amount(
                Amount::of_json_number(b"2").expect("a number"),
            )],
            "decimal",
        );
        reported(
            vec![Said::Array(vec![Said::Bool(true), Said::Bool(false)])],
            "list<bool>",
        );
        assert_eq!(
            issues(decoding),
            vec![
                r#" duplicate_element {"duplicates":{"list":[{"date":"2026-01-31"}]}}"#,
                r#" duplicate_element {"duplicates":{"list":[{"decimal":"2"}]}}"#,
                r#" duplicate_element {"duplicates":{"list":[{"list":[{"bool":true},{"bool":false}]}]}}"#,
            ]
        );
        souther_scope_close(scope);
    }

    #[test]
    fn bytes_that_are_not_a_document_say_where_and_read_as_nothing() {
        let scope = souther_scope_open();
        let decoding = begun("{\"a\":");
        unsafe {
            assert!(souther_decode_root(decoding).is_null());
            souther_decode_end(decoding, ptr::null());
            assert_eq!(souther_decoded_outcome(decoding), DECODED_MALFORMED);
            assert_eq!(souther_decoded_malformed_at(decoding), Count(5));
        }
        souther_scope_close(scope);
    }
}
