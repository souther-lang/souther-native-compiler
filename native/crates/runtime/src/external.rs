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
use std::collections::HashMap;

/// One node of the external form.
#[derive(Debug, PartialEq)]
pub enum Form {
    Null,
    Bool(bool),
    Number(i64),
    /// A `Decimal`, as the value it is, scale and all: how it is written is the writer's to say,
    /// its amount at a boundary and its scale kept in an issue's metadata ([`Written`]).
    Amount(Amount),
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
/// What every comparison would otherwise work out again is worked out once for every member first
/// ([`Prepared`]), and the members are ordered as a permutation of where they stand, so nothing the
/// preparation points at moves while they are compared.
///
/// # Safety
/// `array` is an array this runtime answered and the caller still owns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_order(array: *mut Form) {
    let Form::Array(items) = (unsafe { &mut *array }) else {
        panic!("a form put in order, where a set's members are an array");
    };
    let mut at: Vec<usize> = (0..items.len()).collect();
    {
        let prepared = Prepared::of(items);
        at.sort_by(|one, other| order(&items[*one], &items[*other], &prepared));
    }
    let mut taken: Vec<Option<Form>> = std::mem::take(items).into_iter().map(Some).collect();
    *items = at
        .into_iter()
        .map(|it| taken[it].take().expect("each member is placed once"))
        .collect();
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
    members.sort_by_cached_key(|(key, _)| units(key));
    unsafe { *array = Form::Object(members) };
}

/// A text's UTF-16 code units, whose order is the order the language writes a boundary in (spec
/// §collections) and not the order of its bytes: a unit from E000 up comes after a surrogate, where
/// its UTF-8 comes before.
fn units(bytes: &[u8]) -> Vec<u16> {
    std::str::from_utf8(bytes)
        .expect("a string form holds text")
        .encode_utf16()
        .collect()
}

/// What ordering a set's members asks of each part of them, worked out once: every text's UTF-16
/// code units, and every object's members in ascending key order. An amount is the value it is
/// already, and is read off its form.
/// By where each part stands, which does not move while the members are ordered.
#[derive(Default)]
struct Prepared {
    units: HashMap<*const Vec<u8>, Vec<u16>>,
    by_key: HashMap<*const Form, Vec<usize>>,
}

impl Prepared {
    /// What ordering `members` asks, walked with a stack of its own.
    fn of(members: &[Form]) -> Prepared {
        let mut prepared = Prepared::default();
        let mut left: Vec<&Form> = members.iter().collect();
        while let Some(form) = left.pop() {
            match form {
                Form::String(text) => {
                    placed(&mut prepared.units, text, units(text));
                }
                Form::Array(items) => left.extend(items),
                Form::Object(members) => {
                    for (key, item) in members {
                        placed(&mut prepared.units, key, units(key));
                        left.push(item);
                    }
                    let mut sorted: Vec<usize> = (0..members.len()).collect();
                    sorted.sort_by(|one, other| {
                        prepared.units[&std::ptr::from_ref(&members[*one].0)]
                            .cmp(&prepared.units[&std::ptr::from_ref(&members[*other].0)])
                    });
                    placed(&mut prepared.by_key, form, sorted);
                }
                Form::Null | Form::Bool(_) | Form::Number(_) | Form::Amount(_) => {}
            }
        }
        prepared
    }

    fn units(&self, text: &Vec<u8>) -> &[u16] {
        &self.units[&std::ptr::from_ref(text)]
    }

    fn amount(&self, form: &Form) -> Amount {
        match form {
            Form::Number(value) => Amount::of_int(*value),
            Form::Amount(amount) => amount.clone(),
            other => unreachable!("{other:?} is no number"),
        }
    }
}

/// `value` put in `index` under `key`, which nothing put there before: each part of a form is
/// walked once, so a part met twice is this walk gone wrong.
fn placed<K: std::hash::Hash + Eq + std::fmt::Debug, V>(
    index: &mut HashMap<K, V>,
    key: K,
    value: V,
) {
    match index.entry(key) {
        std::collections::hash_map::Entry::Vacant(it) => {
            it.insert(value);
        }
        std::collections::hash_map::Entry::Occupied(it) => {
            panic!("{:?} was prepared already", it.key())
        }
    }
}

/// Two forms in the order a boundary writes a set's members in (spec §collections): `null`, then
/// `false`, `true`, a number by its amount, a string by its UTF-16 code units, an array element by
/// element with the shorter first where one runs out, and an object by its members read in
/// ascending key order, each key before its value. What each part is compared by is read off
/// `prepared`, which worked it out once.
///
/// Walked with a stack of its own, as a form is written, so how deep two members are is not a
/// question about the native stack.
fn order(one: &Form, other: &Form, prepared: &Prepared) -> Ordering {
    /// What is left to compare, taken last first.
    enum Left<'f> {
        Forms(&'f Form, &'f Form),
        Keys(&'f Vec<u8>, &'f Vec<u8>),
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
    let mut left = vec![Left::Forms(one, other)];
    while let Some(next) = left.pop() {
        let ordered = match next {
            Left::Counts(one, other) => one.cmp(&other),
            Left::Keys(one, other) => prepared.units(one).cmp(prepared.units(other)),
            Left::Forms(one, other) => match (one, other) {
                _ if rank(one) != rank(other) => rank(one).cmp(&rank(other)),
                (Form::Number(one), Form::Number(other)) => one.cmp(other),
                (Form::String(one), Form::String(other)) => {
                    prepared.units(one).cmp(prepared.units(other))
                }
                (Form::Array(one), Form::Array(other)) => {
                    left.push(Left::Counts(one.len(), other.len()));
                    for (one, other) in one.iter().zip(other).rev() {
                        left.push(Left::Forms(one, other));
                    }
                    Ordering::Equal
                }
                (Form::Object(one_members), Form::Object(other_members)) => {
                    left.push(Left::Counts(one_members.len(), other_members.len()));
                    let one_order = &prepared.by_key[&std::ptr::from_ref(one)];
                    let other_order = &prepared.by_key[&std::ptr::from_ref(other)];
                    for (at, also) in one_order.iter().zip(other_order).rev() {
                        let (one_key, one) = &one_members[*at];
                        let (other_key, other) = &other_members[*also];
                        left.push(Left::Forms(one, other));
                        left.push(Left::Keys(one_key, other_key));
                    }
                    Ordering::Equal
                }
                (one, other) if rank(one) == 3 => {
                    prepared.amount(one).compare(&prepared.amount(other))
                }
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
    write(&root, &mut written, Written::AtABoundary);
    string_of(std::str::from_utf8(&written).expect("JSON written of text is text"))
}

/// One thing left to write: a form, a key and its colon, or punctuation.
enum Step<'a> {
    Form(&'a Form),
    Key(&'a [u8]),
    Punctuation(&'static [u8]),
}

/// What a tree is written as JSON for, which decides how a `Decimal` in it is written.
#[derive(Clone, Copy)]
pub(crate) enum Written {
    /// A value crossing a boundary: a `Decimal` as its amount, whatever scale it was read or
    /// worked out at (spec §primitives), as the JVM's boundary writes one.
    AtABoundary,
    /// Raoh's metadata of an issue, which holds each value as the value it is: a `Decimal` at its
    /// scale, as a `BigDecimal` in the JVM's issue is (`1.50`, not `1.5`).
    AsMetadata,
}

/// The tree as JSON, walked with a stack of its own rather than a frame per level, so how deep a
/// value may be is not a question about the native stack.
pub(crate) fn write(root: &Form, out: &mut Vec<u8>, written: Written) {
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
            Step::Form(Form::Amount(amount)) => out.extend_from_slice(
                match written {
                    Written::AtABoundary => amount.external_text(),
                    Written::AsMetadata => amount.scaled_text(),
                }
                .as_bytes(),
            ),
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
        souther_scope_close, souther_scope_open, souther_string_bytes, souther_string_length,
        souther_string_of_utf8,
    };

    fn string(text: &str) -> *mut Text {
        let mut out = std::ptr::null_mut();
        let admitted = unsafe {
            souther_string_of_utf8(text.as_ptr(), crate::Count(text.len() as i64), &mut out)
        };
        assert_eq!(admitted, 1, "test text has a place");
        out
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
            Form::Amount(Amount::of_json_number(b"2.5").expect("a number")),
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
        let scope = souther_scope_open();
        assert_eq!(
            json(array),
            "[null,false,true,-3,2.5,10,\"a\",\"\u{10000}\",\"\u{ff61}\",[1],[1,2],\
             {\"a\":1,\"c\":0},{\"b\":1,\"a\":2}]"
        );
        souther_scope_close(scope);
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
        let scope = souther_scope_open();
        assert_eq!(
            json(array),
            "{\"a\":1,\"b\":2,\"\u{10000}\":3,\"\u{ff61}\":4}"
        );
        souther_scope_close(scope);
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
        let scope = souther_scope_open();
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
        souther_scope_close(scope);
    }

    #[test]
    fn a_truth_is_any_byte_but_nought() {
        let scope = souther_scope_open();
        assert_eq!(json(souther_external_bool(-1)), "true");
        souther_scope_close(scope);
    }

    #[test]
    fn the_ends_of_an_int_are_written_whole() {
        let scope = souther_scope_open();
        assert_eq!(json(souther_external_int(i64::MIN)), "-9223372036854775808");
        assert_eq!(json(souther_external_int(i64::MAX)), "9223372036854775807");
        assert_eq!(json(souther_external_int(-1)), "-1");
        souther_scope_close(scope);
    }

    #[test]
    fn members_stand_in_the_order_they_were_put() {
        let scope = souther_scope_open();
        let written = object(&[
            ("since", souther_external_int(3)),
            ("type", unsafe { souther_external_string(string("Open")) }),
        ]);
        assert_eq!(json(written), r#"{"since":3,"type":"Open"}"#);
        souther_scope_close(scope);
    }

    #[test]
    fn items_stand_in_the_order_they_were_appended_and_nest() {
        let scope = souther_scope_open();
        let array = souther_external_array();
        unsafe {
            souther_external_append(array, souther_external_int(1));
            souther_external_append(array, object(&[("state", souther_external_object())]));
            souther_external_append(array, souther_external_null());
        }
        assert_eq!(json(array), r#"[1,{"state":{}},null]"#);
        souther_scope_close(scope);
    }

    #[test]
    fn what_json_cannot_hold_bare_is_escaped_and_the_rest_is_written_as_it_is() {
        let scope = souther_scope_open();
        let text = "a\"b\\c\nd\re\tf\u{1}g\u{1f}h𠮷￥";
        assert_eq!(
            json(unsafe { souther_external_string(string(text)) }),
            "\"a\\\"b\\\\c\\nd\\re\\tf\\u0001g\\u001fh𠮷￥\""
        );
        assert_eq!(json(unsafe { souther_external_string(string("")) }), "\"\"");
        souther_scope_close(scope);
    }

    /// As deep as a value can be made, and not as deep as a stack happens to be: a tree is written
    /// and dropped without a frame per level.
    #[test]
    fn a_tree_deeper_than_any_stack_is_written_and_dropped() {
        let scope = souther_scope_open();
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
        souther_scope_close(scope);
    }

    #[test]
    fn a_key_is_escaped_the_way_a_string_is() {
        let scope = souther_scope_open();
        let written = object(&[("a\"b", souther_external_null())]);
        assert_eq!(json(written), r#"{"a\"b":null}"#);
        souther_scope_close(scope);
    }
}
