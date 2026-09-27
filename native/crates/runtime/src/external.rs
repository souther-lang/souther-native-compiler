//! What a value is written as at a boundary: a tree of six kinds of node, and JSON written from it.
//!
//! Nothing here knows a Souther type. Which members an object has, whether an absent field is
//! left out, where a discriminator goes and what it says are all decided by the code a compile
//! generates for a declaration, from what the checker settled about it; this only holds what it is
//! handed and writes it out. The ownership each call takes and gives is stated beside the symbols
//! in `souther-native-abi`.

use super::{Text, string_of, text};
use crate::amount::Amount;
use std::cmp::Ordering;

/// One node of the external form.
#[derive(Debug, PartialEq)]
pub enum Form {
    Null,
    Bool(bool),
    Number(i64),
    /// A `Decimal`, as the text its amount is written as.
    Amount(String),
    String(Vec<u8>),
    Array(Vec<Form>),
    /// Members in the order they were put, a key given twice kept twice: the code that builds an
    /// object is what decides its members, and this does not second-guess it.
    Object(Vec<(Vec<u8>, Form)>),
}

pub(crate) fn handed(form: Form) -> *mut Form {
    Box::into_raw(Box::new(form))
}

/// # Safety
/// `form` was answered by one of the constructors here and has not been handed on since.
unsafe fn taken(form: *mut Form) -> Form {
    *unsafe { Box::from_raw(form) }
}

#[unsafe(no_mangle)]
pub extern "C" fn souther_external_null() -> *mut Form {
    handed(Form::Null)
}

#[unsafe(no_mangle)]
pub extern "C" fn souther_external_bool(value: i8) -> *mut Form {
    handed(Form::Bool(value != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn souther_external_int(value: i64) -> *mut Form {
    handed(Form::Number(value))
}

/// # Safety
/// `at` is a string of the runtime's layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_string(at: *const Text) -> *mut Form {
    handed(Form::String(unsafe { text(&at) }.as_bytes().to_vec()))
}

#[unsafe(no_mangle)]
pub extern "C" fn souther_external_array() -> *mut Form {
    handed(Form::Array(Vec::new()))
}

/// # Safety
/// `array` is an array this runtime answered and the caller still owns; `item` is a form the
/// caller owns, and owns no longer once this returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_append(array: *mut Form, item: *mut Form) {
    let item = unsafe { taken(item) };
    match unsafe { &mut *array } {
        Form::Array(items) => items.push(item),
        other => panic!("an item appended to {other:?}, which is not an array"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn souther_external_object() -> *mut Form {
    handed(Form::Object(Vec::new()))
}

/// # Safety
/// `object` is an object this runtime answered and the caller still owns; `key` is a string of the
/// runtime's layout; `item` is a form the caller owns, and owns no longer once this returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_put(
    object: *mut Form,
    key: *const Text,
    item: *mut Form,
) {
    let key = unsafe { text(&key) }.as_bytes().to_vec();
    let item = unsafe { taken(item) };
    match unsafe { &mut *object } {
        Form::Object(members) => members.push((key, item)),
        other => panic!("a member put into {other:?}, which is not an object"),
    }
}

/// Puts a set's members in the order a boundary writes them in ([`order`]).
///
/// # Safety
/// `array` is an array this runtime answered and the caller still owns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_order(array: *mut Form) {
    match unsafe { &mut *array } {
        Form::Array(items) => items.sort_by(order),
        other => panic!("{other:?} put in order, where a set's members are an array"),
    }
}

/// Makes a map's entries, an array of pairs of a key and a value, the object a boundary writes,
/// its members ascending by their keys as [`order`] orders two strings.
///
/// # Safety
/// `array` is an array this runtime answered and the caller still owns, of arrays of two, the first
/// of each a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_entries(array: *mut Form) {
    let entries = match unsafe { &mut *array } {
        Form::Array(items) => std::mem::take(items),
        other => panic!("{other:?} made an object of, where a map's entries are an array"),
    };
    let mut members: Vec<(Vec<u8>, Form)> = entries
        .into_iter()
        .map(|mut entry| match &mut entry {
            Form::Array(pair) if pair.len() == 2 => {
                let value = pair.pop().expect("a pair holds two");
                match &mut pair.pop().expect("a pair holds two") {
                    Form::String(key) => (std::mem::take(key), value),
                    other => panic!("{other:?} as a map's key, which is written as text"),
                }
            }
            other => panic!("{other:?} as a map's entry, which is a pair"),
        })
        .collect();
    members.sort_by(|(one, _), (other, _)| in_units(one, other));
    unsafe { *array = Form::Object(members) };
}

/// Two texts in the order of their UTF-16 code units, which is the order the language writes a
/// boundary in (spec §collections) and not the order of their bytes: a unit from E000 up comes
/// after a surrogate, where its UTF-8 comes before.
fn in_units(one: &[u8], other: &[u8]) -> Ordering {
    let text = |bytes| std::str::from_utf8(bytes).expect("a string form holds text");
    text(one).encode_utf16().cmp(text(other).encode_utf16())
}

/// Two forms in the order a boundary writes a set's members in (spec §collections): `null`, then
/// `false`, `true`, a number by its amount, a string by its UTF-16 code units, an array element by
/// element with the shorter first where one runs out, and an object by its members read in
/// ascending key order, each key before its value.
///
/// Walked with a stack of its own, as a form is written, so how deep two members are is not a
/// question about the native stack.
fn order(one: &Form, other: &Form) -> Ordering {
    /// What is left to compare, taken last first.
    enum Left<'f> {
        Forms(&'f Form, &'f Form),
        Keys(&'f [u8], &'f [u8]),
        Counts(usize, usize),
    }
    fn rank(form: &Form) -> u8 {
        match form {
            Form::Null => 0,
            Form::Bool(false) => 1,
            Form::Bool(true) => 2,
            Form::Number(_) | Form::Amount(_) => 3,
            Form::String(_) => 4,
            Form::Array(_) => 5,
            Form::Object(_) => 6,
        }
    }
    fn amount(form: &Form) -> Amount {
        match form {
            Form::Number(value) => Amount::of_int(*value),
            Form::Amount(written) => Amount::of_json_number(written.as_bytes())
                .expect("an amount is written as a JSON number"),
            other => unreachable!("{other:?} is no number"),
        }
    }
    fn by_key(members: &[(Vec<u8>, Form)]) -> Vec<&(Vec<u8>, Form)> {
        let mut sorted: Vec<_> = members.iter().collect();
        sorted.sort_by(|(one, _), (other, _)| in_units(one, other));
        sorted
    }
    let mut left = vec![Left::Forms(one, other)];
    while let Some(next) = left.pop() {
        let ordered = match next {
            Left::Counts(one, other) => one.cmp(&other),
            Left::Keys(one, other) => in_units(one, other),
            Left::Forms(one, other) => match (one, other) {
                _ if rank(one) != rank(other) => rank(one).cmp(&rank(other)),
                (Form::String(one), Form::String(other)) => in_units(one, other),
                (Form::Array(one), Form::Array(other)) => {
                    left.push(Left::Counts(one.len(), other.len()));
                    for (one, other) in one.iter().zip(other).rev() {
                        left.push(Left::Forms(one, other));
                    }
                    Ordering::Equal
                }
                (Form::Object(one), Form::Object(other)) => {
                    left.push(Left::Counts(one.len(), other.len()));
                    for ((one_key, one), (other_key, other)) in
                        by_key(one).into_iter().zip(by_key(other)).rev()
                    {
                        left.push(Left::Forms(one, other));
                        left.push(Left::Keys(one_key, other_key));
                    }
                    Ordering::Equal
                }
                (one, other) if rank(one) == 3 => amount(one).compare(&amount(other)),
                // A kind with one value, or a truth, which its rank has told apart already.
                _ => Ordering::Equal,
            },
        };
        if ordered != Ordering::Equal {
            return ordered;
        }
    }
    Ordering::Equal
}

/// # Safety
/// `root` is a form the caller owns, and owns no longer once this returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_json(root: *mut Form) -> *mut Text {
    let root = unsafe { taken(root) };
    let mut written = Vec::new();
    write(&root, &mut written);
    string_of(std::str::from_utf8(&written).expect("JSON written of text is text"))
}

/// One thing left to write: a form, a key and its colon, or punctuation.
enum Step<'a> {
    Form(&'a Form),
    Key(&'a [u8]),
    Punctuation(&'static [u8]),
}

/// The tree as JSON, walked with a stack of its own rather than a frame per level, so how deep a
/// value may be is not a question about the native stack.
fn write(root: &Form, out: &mut Vec<u8>) {
    let mut left = vec![Step::Form(root)];
    while let Some(step) = left.pop() {
        match step {
            Step::Punctuation(text) => out.extend_from_slice(text),
            Step::Key(key) => {
                quoted(key, out);
                out.push(b':');
            }
            Step::Form(Form::Null) => out.extend_from_slice(b"null"),
            Step::Form(Form::Bool(true)) => out.extend_from_slice(b"true"),
            Step::Form(Form::Bool(false)) => out.extend_from_slice(b"false"),
            Step::Form(Form::Number(value)) => out.extend_from_slice(value.to_string().as_bytes()),
            Step::Form(Form::Amount(written)) => out.extend_from_slice(written.as_bytes()),
            Step::Form(Form::String(text)) => quoted(text, out),
            // What follows the opening is pushed last-first, so it comes off in the order written.
            Step::Form(Form::Array(items)) => {
                out.push(b'[');
                left.push(Step::Punctuation(b"]"));
                for (at, item) in items.iter().enumerate().rev() {
                    left.push(Step::Form(item));
                    if at > 0 {
                        left.push(Step::Punctuation(b","));
                    }
                }
            }
            Step::Form(Form::Object(members)) => {
                out.push(b'{');
                left.push(Step::Punctuation(b"}"));
                for (at, (key, item)) in members.iter().enumerate().rev() {
                    left.push(Step::Form(item));
                    left.push(Step::Key(key));
                    if at > 0 {
                        left.push(Step::Punctuation(b","));
                    }
                }
            }
        }
    }
}

/// Dropped a level at a time for the reason it is written that way: the children are taken out
/// onto a list before each form goes, so no drop recurses into what it held.
impl Drop for Form {
    fn drop(&mut self) {
        let mut held = Vec::new();
        take_children(self, &mut held);
        while let Some(mut form) = held.pop() {
            take_children(&mut form, &mut held);
        }
    }
}

fn take_children(form: &mut Form, into: &mut Vec<Form>) {
    match form {
        Form::Array(items) => into.append(items),
        Form::Object(members) => into.extend(members.drain(..).map(|(_, item)| item)),
        Form::Null | Form::Bool(_) | Form::Number(_) | Form::Amount(_) | Form::String(_) => {}
    }
}

/// Text as a JSON string. What JSON cannot hold bare — the quote, the backslash, and everything
/// below U+0020 — is escaped, and the rest is written as the UTF-8 it already is.
fn quoted(text: &[u8], out: &mut Vec<u8>) {
    out.push(b'"');
    for &byte in text {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0x00..=0x1f => out.extend_from_slice(format!("\\u{byte:04x}").as_bytes()),
            _ => out.push(byte),
        }
    }
    out.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        souther_mark, souther_reset, souther_string_bytes, souther_string_length,
        souther_string_of_utf8,
    };

    fn string(text: &str) -> *mut Text {
        unsafe { souther_string_of_utf8(text.as_ptr(), crate::Count(text.len() as i64)) }
    }

    /// A set's members in the order the language writes them: by kind, a number by its amount
    /// whichever way it was written, text by UTF-16 code unit, and what holds others by what it
    /// holds, an object read in the order of its keys.
    #[test]
    fn a_sets_members_are_written_in_the_order_of_what_they_are() {
        let members = vec![
            Form::Object(vec![
                (b"b".to_vec(), Form::Number(1)),
                (b"a".to_vec(), Form::Number(2)),
            ]),
            Form::Array(vec![Form::Number(1), Form::Number(2)]),
            Form::Array(vec![Form::Number(1)]),
            Form::String("\u{ff61}".as_bytes().to_vec()),
            Form::String("\u{10000}".as_bytes().to_vec()),
            Form::String(b"a".to_vec()),
            Form::Amount("2.5".to_string()),
            Form::Number(10),
            Form::Number(-3),
            Form::Bool(true),
            Form::Bool(false),
            Form::Null,
            Form::Object(vec![
                (b"a".to_vec(), Form::Number(1)),
                (b"c".to_vec(), Form::Number(0)),
            ]),
        ];
        let array = handed(Form::Array(members));
        unsafe { souther_external_order(array) };
        let mark = souther_mark();
        assert_eq!(
            json(array),
            "[null,false,true,-3,2.5,10,\"a\",\"\u{10000}\",\"\u{ff61}\",[1],[1,2],\
             {\"a\":1,\"c\":0},{\"b\":1,\"a\":2}]"
        );
        souther_reset(mark);
    }

    /// A map's entries, as the object written with its keys ascending.
    #[test]
    fn a_maps_entries_are_written_in_the_order_of_their_keys() {
        let pair =
            |key: &str, value| Form::Array(vec![Form::String(key.as_bytes().to_vec()), value]);
        let array = handed(Form::Array(vec![
            pair("b", Form::Number(2)),
            pair("\u{ff61}", Form::Number(4)),
            pair("\u{10000}", Form::Number(3)),
            pair("a", Form::Number(1)),
        ]));
        unsafe { souther_external_entries(array) };
        let mark = souther_mark();
        assert_eq!(
            json(array),
            "{\"a\":1,\"b\":2,\"\u{10000}\":3,\"\u{ff61}\":4}"
        );
        souther_reset(mark);
    }

    fn json(form: *mut Form) -> String {
        let at = unsafe { souther_external_json(form) };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                souther_string_bytes(at),
                souther_string_length(at).0 as usize,
            )
        };
        String::from_utf8(bytes.to_vec()).expect("what is written is UTF-8")
    }

    fn object(members: &[(&str, *mut Form)]) -> *mut Form {
        let object = souther_external_object();
        for (key, item) in members {
            unsafe { souther_external_put(object, string(key), *item) };
        }
        object
    }

    #[test]
    fn each_kind_is_written_as_the_json_it_is() {
        let mark = souther_mark();
        assert_eq!(json(souther_external_null()), "null");
        assert_eq!(json(souther_external_bool(1)), "true");
        assert_eq!(json(souther_external_bool(0)), "false");
        assert_eq!(json(souther_external_int(42)), "42");
        assert_eq!(
            json(unsafe { souther_external_string(string("abc")) }),
            "\"abc\""
        );
        assert_eq!(json(souther_external_array()), "[]");
        assert_eq!(json(souther_external_object()), "{}");
        souther_reset(mark);
    }

    #[test]
    fn a_truth_is_any_byte_but_nought() {
        let mark = souther_mark();
        assert_eq!(json(souther_external_bool(-1)), "true");
        souther_reset(mark);
    }

    #[test]
    fn the_ends_of_an_int_are_written_whole() {
        let mark = souther_mark();
        assert_eq!(json(souther_external_int(i64::MIN)), "-9223372036854775808");
        assert_eq!(json(souther_external_int(i64::MAX)), "9223372036854775807");
        assert_eq!(json(souther_external_int(-1)), "-1");
        souther_reset(mark);
    }

    #[test]
    fn members_stand_in_the_order_they_were_put() {
        let mark = souther_mark();
        let written = object(&[
            ("since", souther_external_int(3)),
            ("type", unsafe { souther_external_string(string("Open")) }),
        ]);
        assert_eq!(json(written), r#"{"since":3,"type":"Open"}"#);
        souther_reset(mark);
    }

    #[test]
    fn items_stand_in_the_order_they_were_appended_and_nest() {
        let mark = souther_mark();
        let array = souther_external_array();
        unsafe {
            souther_external_append(array, souther_external_int(1));
            souther_external_append(array, object(&[("state", souther_external_object())]));
            souther_external_append(array, souther_external_null());
        }
        assert_eq!(json(array), r#"[1,{"state":{}},null]"#);
        souther_reset(mark);
    }

    #[test]
    fn what_json_cannot_hold_bare_is_escaped_and_the_rest_is_written_as_it_is() {
        let mark = souther_mark();
        let text = "a\"b\\c\nd\re\tf\u{1}g\u{1f}h𠮷￥";
        assert_eq!(
            json(unsafe { souther_external_string(string(text)) }),
            "\"a\\\"b\\\\c\\nd\\re\\tf\\u0001g\\u001fh𠮷￥\""
        );
        assert_eq!(json(unsafe { souther_external_string(string("")) }), "\"\"");
        souther_reset(mark);
    }

    /// As deep as a value can be made, and not as deep as a stack happens to be: a tree is written
    /// and dropped without a frame per level.
    #[test]
    fn a_tree_deeper_than_any_stack_is_written_and_dropped() {
        let mark = souther_mark();
        let depth = 1_000_000;
        let root = souther_external_array();
        let mut innermost = root;
        for _ in 0..depth {
            let next = souther_external_array();
            unsafe { souther_external_append(innermost, next) };
            innermost = match unsafe { &mut *innermost } {
                Form::Array(items) => items.last_mut().expect("just appended") as *mut Form,
                _ => unreachable!(),
            };
        }
        let written = json(root);
        assert_eq!(written.len(), 2 * (depth + 1));
        assert!(written.starts_with("[[[") && written.ends_with("]]]"));
        souther_reset(mark);
    }

    #[test]
    fn a_key_is_escaped_the_way_a_string_is() {
        let mark = souther_mark();
        let written = object(&[("a\"b", souther_external_null())]);
        assert_eq!(json(written), r#"{"a\"b":null}"#);
        souther_reset(mark);
    }
}
