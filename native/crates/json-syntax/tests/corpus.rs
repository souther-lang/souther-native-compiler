//! Documents and what each is read as, written as data so that the wasm runtime's reader can be
//! held to the same rows the day both read this crate (#17).
//!
//! Where the wasm runtime's own reader answers a row differently today, the row says so. Those are
//! the places the two copies have already come apart, and the reason #17 asks for one.

use souther_json_syntax::{DEEPEST, Event, Malformed};

/// The events a document is read as, written one per word, or where it stopped being one.
fn read(document: &[u8]) -> Result<String, usize> {
    let mut said = Vec::new();
    souther_json_syntax::read(document, |event| {
        said.push(match event {
            Event::Null => "null".to_string(),
            Event::Bool(truth) => truth.to_string(),
            Event::Number(written) => format!("#{}", String::from_utf8_lossy(written)),
            Event::String(text) => {
                format!("'{}'", String::from_utf8(text.bytes().collect()).unwrap())
            }
            Event::Key(text) => format!("{}:", String::from_utf8(text.bytes().collect()).unwrap()),
            Event::BeginArray => "[".to_string(),
            Event::EndArray => "]".to_string(),
            Event::BeginObject => "{".to_string(),
            Event::EndObject => "}".to_string(),
        })
    })
    .map_err(|Malformed { at }| at)?;
    Ok(said.join(" "))
}

/// Documents that are one, and what they say.
const READ: &[(&str, &str)] = &[
    ("null", "null"),
    (" \t\r\ntrue \n", "true"),
    ("false", "false"),
    ("0", "#0"),
    ("-0", "#-0"),
    ("12", "#12"),
    ("1.0", "#1.0"),
    ("1e0", "#1e0"),
    ("-1.25E+3", "#-1.25E+3"),
    ("9223372036854775808", "#9223372036854775808"),
    ("\"\"", "''"),
    ("\"a\\\"b\\\\c\\/d\"", "'a\"b\\c/d'"),
    ("\"\\b\\f\\n\\r\\t\"", "'\u{8}\u{c}\n\r\t'"),
    ("\"\\u00e9\\u3042\"", "'é\u{3042}'"),
    ("\"\\ud842\\udfb7\"", "'𠮷'"),
    ("\"𠮷￥\"", "'𠮷￥'"),
    ("[]", "[ ]"),
    ("[ 1 , [ ] , { } ]", "[ #1 [ ] { } ]"),
    ("{}", "{ }"),
    (
        "{\"a\":1,\"b\":{\"c\":[null]}}",
        "{ a: #1 b: { c: [ null ] } }",
    ),
    // Kept twice, in the order written: which of the two a reader takes is not this crate's to say.
    ("{\"a\":1,\"a\":2}", "{ a: #1 a: #2 }"),
    ("{\"a/b~c\":true}", "{ a/b~c: true }"),
];

/// Documents that are not one, and the offset each stopped being one at.
const REFUSED: &[(&str, usize)] = &[
    ("", 0),
    ("   ", 3),
    ("nul", 3),
    ("nulls", 4),
    ("True", 0),
    ("01", 1),
    ("-", 1),
    ("1.", 2),
    (".5", 0),
    ("1e", 2),
    ("+1", 0),
    ("[1,]", 3),
    ("[1 2]", 3),
    ("{\"a\"}", 4),
    ("{\"a\":1,}", 7),
    ("{a:1}", 1),
    ("{\"a\":1", 6),
    ("[", 1),
    ("]", 0),
    ("1 2", 2),
    // A whole value followed by anything but whitespace is not one document, however whole the
    // first value was.
    ("1x", 1),
    ("{}x", 2),
    ("{\"a\":1}{\"b\":2}", 7),
    ("[] []", 3),
    ("\"open", 5),
    ("\"\\x\"", 2),
    ("\"\\u12\"", 3),
    // Half a surrogate pair spells no character.
    ("\"\\ud842\"", 7),
    ("\"\\udfb7\"", 1),
    ("\"\\ud842\\u0041\"", 9),
    // A control character written bare. The wasm runtime reads this as the character.
    ("\"a\nb\"", 2),
    ("\"\u{1}\"", 1),
];

#[test]
fn a_document_is_read_as_the_pieces_it_was_written_as() {
    for (document, said) in READ {
        assert_eq!(
            read(document.as_bytes()),
            Ok(said.to_string()),
            "{document}"
        );
    }
}

#[test]
fn a_document_that_is_not_one_says_where_it_stopped_being_one() {
    for (document, at) in REFUSED {
        assert_eq!(read(document.as_bytes()), Err(*at), "{document:?}");
    }
}

/// Bytes that are not UTF-8 are not text, escaped or not. The wasm runtime reads them as they are.
#[test]
fn bytes_that_are_not_utf_8_refuse_the_document() {
    assert_eq!(read(b"\"a\xffb\""), Err(2));
    assert_eq!(read(b"\"\xe3\x81\""), Err(1));
    assert_eq!(read(b"[\"ok\", \"\xc0\xaf\"]"), Err(8));
}

/// Nested as deep as a document may be is read; one deeper is refused where it opens.
///
/// Counted by what is open at once and not by how many containers a document holds. The wasm
/// runtime counts the second, so a document holding more than two hundred arrays side by side is
/// refused there and read here.
#[test]
fn how_deep_a_document_may_be_is_how_many_containers_are_open_at_once() {
    let deepest = format!("{}{}", "[".repeat(DEEPEST), "]".repeat(DEEPEST));
    assert!(read(deepest.as_bytes()).is_ok());

    let deeper = format!("{}{}", "[".repeat(DEEPEST + 1), "]".repeat(DEEPEST + 1));
    assert_eq!(read(deeper.as_bytes()), Err(DEEPEST));

    let wide = format!("[{}]", vec!["[]"; DEEPEST * 2].join(","));
    assert!(read(wide.as_bytes()).is_ok());
}

#[test]
fn a_text_says_the_bytes_it_unescapes_to() {
    let mut read = None;
    souther_json_syntax::read(br#""a\u00e9\n""#, |event| read = Some(event)).unwrap();
    let Some(Event::String(text)) = read else {
        panic!("a string");
    };
    assert!(text.says("aé\n".as_bytes()));
    assert!(!text.says(b"a"));
    assert!(!text.says("aé\nx".as_bytes()));
    assert_eq!(text.written(), br"a\u00e9\n");
}
