//! A document as the tree its events build, for a reader that asks for a member by its key.
//!
//! What the document says and nothing more: a number is the digits it was written with, a string
//! the text its escapes spell, an object its members in the order they were written with a key
//! written twice kept twice. What is a document at all is `souther-json-syntax`'s to say, and what a
//! place in one is read as is the generated reader's; this only holds the one for the other.
//!
//! On the heap and not in the arena. A tree is held for the length of one decode and dropped when
//! it ends, and a node of it is never what a Souther value is made of: a string read from one is
//! copied into the arena as a string of the runtime's own layout.
//!
//! Two kinds of document are read, and they differ in one place. Text a host was handed says of
//! every container whether it is an object or an array, and that is part of what it says. A value a
//! host built does not always: in PHP, and in any host whose one container is an ordered map, a
//! list is the map whose keys are its indices, and an empty list and an empty object are the same
//! value. Such a value is written out with every container as an object keyed as the host keyed it
//! ([`Form::HostValue`]), which loses nothing, and read here as a [`Node::Keyed`]: an object always,
//! and an array where its keys are its indices. Which the place is taken as is then the reader's to
//! say, which is the one that knows what the position holds.

use souther_json_syntax::{Event, Malformed, read};

/// What a document is the writing of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    /// Text in the external form, which says of every container which kind it is.
    Text,
    /// A value a host built out of ordered maps, every one of them written as an object.
    HostValue,
}

/// One place in a document.
#[derive(Debug, PartialEq)]
pub(crate) enum Node {
    Null,
    Bool(bool),
    Number(Box<[u8]>),
    String(Box<[u8]>),
    Array(Vec<Node>),
    Object(Vec<(Box<[u8]>, Node)>),
    /// A host's ordered map: an object, and an array too where its keys are `0`, `1` and on, in
    /// that order. Made only by reading a [`Form::HostValue`].
    Keyed(Vec<(Box<[u8]>, Node)>),
}

impl Node {
    /// What kind of place this is, as an issue names what was found where something else was
    /// wanted. A host's map that is no list was taken as an object, which is what it is named.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Node::Null => "null",
            Node::Bool(_) => "boolean",
            Node::Number(_) => "number",
            Node::String(_) => "string",
            Node::Array(_) => "array",
            Node::Object(_) | Node::Keyed(_) => "object",
        }
    }

    /// Whether this is an object.
    pub(crate) fn is_object(&self) -> bool {
        matches!(self, Node::Object(_) | Node::Keyed(_))
    }

    /// How many elements this holds where it is an array, and none where it is not.
    pub(crate) fn length(&self) -> Option<usize> {
        match self {
            Node::Array(items) => Some(items.len()),
            Node::Keyed(members) => members
                .iter()
                .enumerate()
                .all(|(at, (key, _))| **key == *at.to_string().as_bytes())
                .then_some(members.len()),
            _ => None,
        }
    }

    /// The element at `index` of this array, or none where this is no array or holds no such
    /// element.
    pub(crate) fn element(&self, index: usize) -> Option<&Node> {
        self.length().filter(|length| index < *length)?;
        match self {
            Node::Array(items) => items.get(index),
            Node::Keyed(members) => members.get(index).map(|(_, value)| value),
            _ => None,
        }
    }

    /// The member of an object written under `key`, the first where the key was written more than
    /// once; none where there is no such member, or where this is not an object.
    ///
    /// The first, which is what the wasm runtime answers. A key written twice is a document saying
    /// two things about one field, and which of them it meant is not in it; what is decided here is
    /// only that both runtimes pick the same one.
    pub(crate) fn member(&self, key: &[u8]) -> Option<&Node> {
        match self {
            Node::Object(members) | Node::Keyed(members) => members
                .iter()
                .find(|(written, _)| **written == *key)
                .map(|(_, value)| value),
            _ => None,
        }
    }
}

/// A container whose closing has not been read yet, holding what it has been given so far.
enum Open {
    Array(Vec<Node>),
    Object(Vec<(Box<[u8]>, Node)>, Option<Box<[u8]>>),
}

/// The document `bytes` are, written as `form` says, or where they stopped being one.
///
/// Built with a stack of its own rather than a frame per level, so reading a document takes no
/// more native stack however it nests. How deep one may be is the parser's to limit, and whether
/// the bytes are one document at all is the parser's to say: the root built here is answered only
/// once [`read`] has answered that nothing follows it.
pub(crate) fn parsed(bytes: &[u8], form: Form) -> Result<Node, Malformed> {
    let mut open: Vec<Open> = Vec::new();
    let mut root = None;
    read(bytes, |event| {
        let whole = match event {
            Event::Null => Node::Null,
            Event::Bool(truth) => Node::Bool(truth),
            Event::Number(written) => Node::Number(written.into()),
            Event::String(text) => Node::String(text.bytes().collect()),
            Event::BeginArray => {
                open.push(Open::Array(Vec::new()));
                return;
            }
            Event::BeginObject => {
                open.push(Open::Object(Vec::new(), None));
                return;
            }
            Event::Key(text) => {
                match open.last_mut() {
                    Some(Open::Object(_, key)) => *key = Some(text.bytes().collect()),
                    _ => unreachable!("the parser answers a key only inside an object"),
                }
                return;
            }
            Event::EndArray => match open.pop() {
                Some(Open::Array(items)) => Node::Array(items),
                _ => unreachable!("the parser closes only the array that is open"),
            },
            Event::EndObject => match (open.pop(), form) {
                (Some(Open::Object(members, _)), Form::Text) => Node::Object(members),
                (Some(Open::Object(members, _)), Form::HostValue) => Node::Keyed(members),
                _ => unreachable!("the parser closes only the object that is open"),
            },
        };
        match open.last_mut() {
            None => root = Some(whole),
            Some(Open::Array(items)) => items.push(whole),
            Some(Open::Object(members, key)) => {
                let key = key
                    .take()
                    .expect("the parser answers a key before a member's value");
                members.push((key, whole));
            }
        }
    })?;
    Ok(root.expect("a document the parser read whole has a root"))
}

#[cfg(test)]
mod tests {
    use super::{Form, Node};

    fn parsed(bytes: &[u8]) -> Result<Node, souther_json_syntax::Malformed> {
        super::parsed(bytes, Form::Text)
    }

    fn host(bytes: &[u8]) -> Node {
        super::parsed(bytes, Form::HostValue).unwrap()
    }

    #[test]
    fn a_document_is_the_tree_it_was_written_as() {
        let read = parsed(br#"{"a":[1,"x",null],"b":{"c":true}}"#).unwrap();
        assert_eq!(
            read,
            Node::Object(vec![
                (
                    b"a".as_slice().into(),
                    Node::Array(vec![
                        Node::Number(b"1".as_slice().into()),
                        Node::String(b"x".as_slice().into()),
                        Node::Null,
                    ])
                ),
                (
                    b"b".as_slice().into(),
                    Node::Object(vec![(b"c".as_slice().into(), Node::Bool(true))])
                ),
            ])
        );
    }

    #[test]
    fn a_key_written_twice_is_read_as_the_first() {
        let read = parsed(br#"{"a":1,"a":2}"#).unwrap();
        assert_eq!(
            read.member(b"a"),
            Some(&Node::Number(b"1".as_slice().into()))
        );
        assert_eq!(read.member(b"b"), None);
    }

    #[test]
    fn where_a_document_stops_being_one_is_answered() {
        assert_eq!(parsed(b"[1,").unwrap_err().at, 3);
    }

    /// A whole value is not a document while anything but whitespace follows it, however early
    /// the tree was whole.
    #[test]
    fn a_value_followed_by_more_is_not_a_document() {
        assert_eq!(parsed(b"1 2").unwrap_err().at, 2);
        assert_eq!(parsed(b"1x").unwrap_err().at, 1);
        assert_eq!(parsed(b"nulls").unwrap_err().at, 4);
        assert_eq!(parsed(b"{}x").unwrap_err().at, 2);
        assert_eq!(parsed(br#"{"a":1}{"b":2}"#).unwrap_err().at, 7);
        assert_eq!(parsed(b" {} \n").unwrap(), Node::Object(Vec::new()));
    }

    /// Text says which kind a container is, and an empty object is no array.
    #[test]
    fn text_says_which_kind_every_container_is() {
        assert!(parsed(b"{}").unwrap().is_object());
        assert_eq!(parsed(b"{}").unwrap().length(), None);
        assert_eq!(parsed(br#"{"0":1}"#).unwrap().length(), None);
        assert!(!parsed(b"[]").unwrap().is_object());
        assert_eq!(parsed(b"[]").unwrap().length(), Some(0));
    }

    /// A host's map is an object, and an array where its keys are its indices in order: the empty
    /// one is both, as the empty list and the empty object are one value to the host.
    #[test]
    fn a_hosts_map_is_an_array_where_its_keys_are_its_indices() {
        let empty = host(b"{}");
        assert!(empty.is_object());
        assert_eq!(empty.length(), Some(0));

        let listed = host(br#"{"0":"a","1":"b"}"#);
        assert!(listed.is_object());
        assert_eq!(listed.length(), Some(2));
        assert_eq!(
            listed.element(1),
            Some(&Node::String(b"b".as_slice().into()))
        );
        assert_eq!(listed.element(2), None);
        assert_eq!(
            listed.member(b"0"),
            Some(&Node::String(b"a".as_slice().into()))
        );

        assert_eq!(host(br#"{"1":"a","0":"b"}"#).length(), None);
        assert_eq!(host(br#"{"0":"a","2":"b"}"#).length(), None);
        assert_eq!(host(br#"{"name":"a"}"#).length(), None);
        assert_eq!(host(br#"{"name":"a"}"#).element(0), None);
        assert_eq!(
            host(br#"{"name":{}}"#).member(b"name").unwrap().length(),
            Some(0)
        );
    }
}
