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

use souther_json_syntax::{Event, Malformed, Parser};

/// One place in a document.
#[derive(Debug, PartialEq)]
pub(crate) enum Node {
    Null,
    Bool(bool),
    Number(Box<[u8]>),
    String(Box<[u8]>),
    Array(Vec<Node>),
    Object(Vec<(Box<[u8]>, Node)>),
}

impl Node {
    /// What kind of place this is, as an issue names what was found where something else was
    /// wanted.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Node::Null => "null",
            Node::Bool(_) => "boolean",
            Node::Number(_) => "number",
            Node::String(_) => "string",
            Node::Array(_) => "array",
            Node::Object(_) => "object",
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
            Node::Object(members) => members
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

/// The document `bytes` are, or where they stopped being one.
///
/// Built with a stack of its own rather than a frame per level, so reading a document takes no
/// more native stack however it nests. How deep one may be is the parser's to limit.
pub(crate) fn parsed(bytes: &[u8]) -> Result<Node, Malformed> {
    let mut open: Vec<Open> = Vec::new();
    for event in Parser::new(bytes) {
        let whole = match event? {
            Event::Null => Node::Null,
            Event::Bool(truth) => Node::Bool(truth),
            Event::Number(written) => Node::Number(written.into()),
            Event::String(text) => Node::String(text.bytes().collect()),
            Event::BeginArray => {
                open.push(Open::Array(Vec::new()));
                continue;
            }
            Event::BeginObject => {
                open.push(Open::Object(Vec::new(), None));
                continue;
            }
            Event::Key(text) => {
                match open.last_mut() {
                    Some(Open::Object(_, key)) => *key = Some(text.bytes().collect()),
                    _ => unreachable!("the parser answers a key only inside an object"),
                }
                continue;
            }
            Event::EndArray => match open.pop() {
                Some(Open::Array(items)) => Node::Array(items),
                _ => unreachable!("the parser closes only the array that is open"),
            },
            Event::EndObject => match open.pop() {
                Some(Open::Object(members, _)) => Node::Object(members),
                _ => unreachable!("the parser closes only the object that is open"),
            },
        };
        match open.last_mut() {
            None => return Ok(whole),
            Some(Open::Array(items)) => items.push(whole),
            Some(Open::Object(members, key)) => {
                let key = key
                    .take()
                    .expect("the parser answers a key before a member's value");
                members.push((key, whole));
            }
        }
    }
    unreachable!("the parser answers a whole value or where it stopped before it ends")
}

#[cfg(test)]
mod tests {
    use super::{Node, parsed};

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
}
