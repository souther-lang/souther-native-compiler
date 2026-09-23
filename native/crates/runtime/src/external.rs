//! What a value is written as at a boundary: a tree of six kinds of node, and JSON written from it.
//!
//! Nothing here knows a Souther type. Which members an object has, whether an absent field is
//! left out, where a discriminator goes and what it says are all decided by the code a compile
//! generates for a declaration, from what the checker settled about it; this only holds what it is
//! handed and writes it out. The ownership each call takes and gives is stated beside the symbols
//! in `souther-native-abi`.

use super::{room_for_a_string, text};
use souther_native_abi::TEXT_BYTES;

/// One node of the external form.
#[derive(Debug, PartialEq)]
pub enum Form {
    Null,
    Bool(bool),
    Number(i64),
    String(Vec<u8>),
    Array(Vec<Form>),
    /// Members in the order they were put, a key given twice kept twice: the code that builds an
    /// object is what decides its members, and this does not second-guess it.
    Object(Vec<(Vec<u8>, Form)>),
}

fn handed(form: Form) -> *mut Form {
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
pub unsafe extern "C" fn souther_external_string(at: *const u8) -> *mut Form {
    handed(Form::String(unsafe { text(at) }.to_vec()))
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
pub unsafe extern "C" fn souther_external_put(object: *mut Form, key: *const u8, item: *mut Form) {
    let key = unsafe { text(key) }.to_vec();
    let item = unsafe { taken(item) };
    match unsafe { &mut *object } {
        Form::Object(members) => members.push((key, item)),
        other => panic!("a member put into {other:?}, which is not an object"),
    }
}

/// # Safety
/// `root` is a form the caller owns, and owns no longer once this returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_json(root: *mut Form) -> *mut u8 {
    let root = unsafe { taken(root) };
    let mut written = Vec::new();
    write(&root, &mut written);
    let at = room_for_a_string(written.len());
    unsafe {
        at.offset(TEXT_BYTES as isize)
            .copy_from_nonoverlapping(written.as_ptr(), written.len())
    };
    at
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
        Form::Null | Form::Bool(_) | Form::Number(_) | Form::String(_) => {}
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

    fn string(text: &str) -> *mut u8 {
        unsafe { souther_string_of_utf8(text.as_ptr(), text.len() as i64) }
    }

    fn json(form: *mut Form) -> String {
        let at = unsafe { souther_external_json(form) };
        let bytes = unsafe {
            std::slice::from_raw_parts(souther_string_bytes(at), souther_string_length(at) as usize)
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
