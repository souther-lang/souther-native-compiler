//! JSON as a run of bytes and nothing else: which of them make one document, and what each piece
//! of it is.
//!
//! Nothing here knows a Souther type, an arena or a runtime. What a document is read as — whether
//! `1.0` is an `Int`, which member a field is — is decided by whoever reads the events this answers,
//! and where what it reads is kept is theirs too. That is what lets this move, as it is, into what
//! both runtimes read (#17): it takes a slice, answers events that borrow from it, and allocates
//! nothing.
//!
//! What a document is, is RFC 8259, with one limit of this reader's own: how deep one may be nested
//! ([`DEEPEST`]).
//!
//! - Text is UTF-8, and bytes that are not refuse the document. A string a reader is handed is text
//!   and never a run of bytes it has to check again.
//! - A character below U+0020 is written escaped inside a string, and one written bare refuses the
//!   document.
//! - A `\u` escape of half a surrogate pair is written with its other half straight after it, and one
//!   written alone refuses the document: it spells no character.
//! - A number is kept as it was written. `1`, `1.0` and `1e0` are three spellings, and which one a
//!   document chose is something a reader may want to know — the scale of a `Decimal` is exactly
//!   that — so nothing here reads a number as an amount.
//! - An object's members are answered in the order they were written, a key written twice twice.
//!   Which of the two a reader takes is the reader's to say.

#![no_std]

/// How deep one document may be nested, counting every array and object that is open at once.
///
/// A reader of the events walks into each level as it opens, and one that walks with a frame per
/// level — as a reader compiled per declaration does — is bounded by its stack. Past this the
/// document is refused as any other that is not one document is, so that how deep a document may
/// be is a number written down and not how much stack happened to be left.
pub const DEEPEST: usize = 200;

/// One piece of a document, in the order it was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    Null,
    Bool(bool),
    /// The number as it was written: a minus, digits, a fraction, an exponent, whichever of those
    /// the document wrote.
    Number(&'a [u8]),
    /// A string standing as a value.
    String(Text<'a>),
    BeginArray,
    EndArray,
    BeginObject,
    /// A member's key. The member's value is the next value answered.
    Key(Text<'a>),
    EndObject,
}

/// Where a document stopped being one: the offset of the byte that could not be read as the next
/// piece, or the length of the document where it ended before it was whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed {
    pub at: usize,
}

/// A string's text as the document wrote it, between the quotes and with its escapes, already held
/// to be text: every escape is one JSON has, and what is not escaped is UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Text<'a> {
    written: &'a [u8],
}

impl<'a> Text<'a> {
    /// The bytes between the quotes, escapes and all.
    pub fn written(&self) -> &'a [u8] {
        self.written
    }

    /// The UTF-8 the text says, an escape read as the character it stands for.
    pub fn bytes(&self) -> Unescaped<'a> {
        Unescaped {
            written: self.written,
            at: 0,
            pending: [0; 4],
            from: 0,
            to: 0,
        }
    }

    /// Whether the text says exactly these bytes.
    pub fn says(&self, bytes: &[u8]) -> bool {
        let mut said = self.bytes();
        for &byte in bytes {
            if said.next() != Some(byte) {
                return false;
            }
        }
        said.next().is_none()
    }
}

/// The UTF-8 a [`Text`] says, one byte at a time.
#[derive(Debug, Clone)]
pub struct Unescaped<'a> {
    written: &'a [u8],
    at: usize,
    /// What an escape stood for and has not been answered yet: `pending[from..to]`.
    pending: [u8; 4],
    from: usize,
    to: usize,
}

impl Iterator for Unescaped<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.from < self.to {
            self.from += 1;
            return Some(self.pending[self.from - 1]);
        }
        let byte = *self.written.get(self.at)?;
        self.at += 1;
        if byte != b'\\' {
            return Some(byte);
        }
        // The text was held to be well formed when it was read, so what follows a backslash is one
        // of the escapes below, and a `\u` of a high surrogate is followed by its low half.
        let escaped = self.written[self.at];
        self.at += 1;
        let plain = match escaped {
            b'b' => 0x08,
            b'f' => 0x0c,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'u' => {
                let mut point = hex4(&self.written[self.at..]).expect("held to be four digits");
                self.at += 4;
                if (0xd800..0xdc00).contains(&point) {
                    let low = hex4(&self.written[self.at + 2..]).expect("held to be a low half");
                    self.at += 6;
                    point = 0x10000 + ((point - 0xd800) << 10) + (low - 0xdc00);
                }
                let character = char::from_u32(point).expect("held to be a character");
                let written = character.encode_utf8(&mut self.pending).len();
                self.from = 1;
                self.to = written;
                return Some(self.pending[0]);
            }
            // `"`, `\` and `/` stand for themselves.
            other => other,
        };
        Some(plain)
    }
}

/// Four hexadecimal digits as the number they write, or `None` where they are not four of them.
fn hex4(bytes: &[u8]) -> Option<u32> {
    let digits = bytes.get(..4)?;
    let mut value = 0;
    for &digit in digits {
        let nibble = match digit {
            b'0'..=b'9' => digit - b'0',
            b'a'..=b'f' => digit - b'a' + 10,
            b'A'..=b'F' => digit - b'A' + 10,
            _ => return None,
        };
        value = (value << 4) | u32::from(nibble);
    }
    Some(value)
}

/// What the reader is ready to read next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    /// A value. `closes` is whether the array it would stand in may close here instead, which it
    /// may straight after it opened.
    Value { closes: bool },
    /// A member's key, or the object closing where `closes` says it may.
    Key { closes: bool },
    /// What follows a value: a comma or a close inside a container, the end at the top.
    After,
    /// Nothing: the document was answered whole, or refused.
    Done,
}

/// Reads one document as the events it is written as.
///
/// An iterator of results: every event in order, then `None`, where the document is one; the
/// events up to where it stopped being one, then one [`Malformed`] and `None`, where it is not. A
/// reader that stops at the first error is handed everything it needs to say where.
#[derive(Debug, Clone)]
pub struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
    next: Next,
    /// Which of the open containers is an object, innermost last: `objects[..depth]`.
    objects: [bool; DEEPEST],
    depth: usize,
}

impl<'a> Parser<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Parser {
            bytes,
            at: 0,
            next: Next::Value { closes: false },
            objects: [false; DEEPEST],
            depth: 0,
        }
    }

    fn malformed(&mut self, at: usize) -> Option<Result<Event<'a>, Malformed>> {
        self.next = Next::Done;
        Some(Err(Malformed { at }))
    }

    fn spaces(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.at) {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    /// Where the reader goes once a value is whole.
    fn after_a_value(&mut self) {
        self.next = Next::After;
    }

    fn open(&mut self, object: bool) -> Result<(), Malformed> {
        if self.depth == DEEPEST {
            return Err(Malformed { at: self.at });
        }
        self.objects[self.depth] = object;
        self.depth += 1;
        self.at += 1;
        self.next = if object {
            Next::Key { closes: true }
        } else {
            Next::Value { closes: true }
        };
        Ok(())
    }

    fn close(&mut self) {
        self.depth -= 1;
        self.at += 1;
        self.after_a_value();
    }

    fn value(&mut self) -> Result<Event<'a>, Malformed> {
        let start = self.at;
        match self.peek() {
            Some(b'n') => self.keyword(b"null", Event::Null),
            Some(b't') => self.keyword(b"true", Event::Bool(true)),
            Some(b'f') => self.keyword(b"false", Event::Bool(false)),
            Some(b'"') => {
                let text = self.string()?;
                self.after_a_value();
                Ok(Event::String(text))
            }
            Some(b'[') => {
                self.open(false)?;
                Ok(Event::BeginArray)
            }
            Some(b'{') => {
                self.open(true)?;
                Ok(Event::BeginObject)
            }
            Some(b'-' | b'0'..=b'9') => {
                self.number()?;
                self.after_a_value();
                Ok(Event::Number(&self.bytes[start..self.at]))
            }
            _ => Err(Malformed { at: start }),
        }
    }

    fn keyword(&mut self, word: &[u8], event: Event<'a>) -> Result<Event<'a>, Malformed> {
        for &expected in word {
            if self.peek() != Some(expected) {
                return Err(Malformed { at: self.at });
            }
            self.at += 1;
        }
        self.after_a_value();
        Ok(event)
    }

    /// `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`. What stands after it is the next
    /// piece's to be, so `01` is a `0` followed by something that is not a comma.
    fn number(&mut self) -> Result<(), Malformed> {
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek() {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => self.digits()?,
            _ => return Err(Malformed { at: self.at }),
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            self.digits()?;
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.at += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.at += 1;
            }
            self.digits()?;
        }
        Ok(())
    }

    fn digits(&mut self) -> Result<(), Malformed> {
        let start = self.at;
        while let Some(b'0'..=b'9') = self.peek() {
            self.at += 1;
        }
        if self.at == start {
            return Err(Malformed { at: self.at });
        }
        Ok(())
    }

    /// A string, from its opening quote to past its closing one.
    fn string(&mut self) -> Result<Text<'a>, Malformed> {
        self.at += 1;
        let start = self.at;
        loop {
            let Some(byte) = self.peek() else {
                return Err(Malformed { at: self.at });
            };
            match byte {
                b'"' => break,
                b'\\' => self.escape()?,
                0x00..=0x1f => return Err(Malformed { at: self.at }),
                _ => self.at += 1,
            }
        }
        let written = &self.bytes[start..self.at];
        // Everything an escape wrote is ASCII, so whether the run is UTF-8 is whether what was
        // written bare is.
        if let Err(error) = core::str::from_utf8(written) {
            return Err(Malformed {
                at: start + error.valid_up_to(),
            });
        }
        self.at += 1;
        Ok(Text { written })
    }

    fn escape(&mut self) -> Result<(), Malformed> {
        let backslash = self.at;
        self.at += 1;
        match self.peek() {
            Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                self.at += 1;
                Ok(())
            }
            Some(b'u') => {
                self.at += 1;
                let point = self.hex4()?;
                if (0xd800..0xdc00).contains(&point) {
                    if self.bytes.get(self.at..self.at + 2) != Some(b"\\u") {
                        return Err(Malformed { at: self.at });
                    }
                    self.at += 2;
                    let low_at = self.at;
                    let low = self.hex4()?;
                    if !(0xdc00..0xe000).contains(&low) {
                        return Err(Malformed { at: low_at });
                    }
                } else if (0xdc00..0xe000).contains(&point) {
                    return Err(Malformed { at: backslash });
                }
                Ok(())
            }
            _ => Err(Malformed { at: self.at }),
        }
    }

    fn hex4(&mut self) -> Result<u32, Malformed> {
        let at = self.at;
        let point = hex4(&self.bytes[at..]).ok_or(Malformed { at })?;
        self.at += 4;
        Ok(point)
    }

    fn step(&mut self) -> Option<Result<Event<'a>, Malformed>> {
        self.spaces();
        let answer = match self.next {
            Next::Done => return None,
            Next::Value { closes } => {
                if closes && self.peek() == Some(b']') {
                    self.close();
                    Ok(Event::EndArray)
                } else {
                    self.value()
                }
            }
            Next::Key { closes } => match self.peek() {
                Some(b'}') if closes => {
                    self.close();
                    Ok(Event::EndObject)
                }
                Some(b'"') => self.string().and_then(|key| {
                    self.spaces();
                    if self.peek() != Some(b':') {
                        return Err(Malformed { at: self.at });
                    }
                    self.at += 1;
                    self.next = Next::Value { closes: false };
                    Ok(Event::Key(key))
                }),
                _ => Err(Malformed { at: self.at }),
            },
            Next::After => {
                if self.depth == 0 {
                    return if self.at == self.bytes.len() {
                        self.next = Next::Done;
                        None
                    } else {
                        self.malformed(self.at)
                    };
                }
                let object = self.objects[self.depth - 1];
                match (self.peek(), object) {
                    (Some(b','), _) => {
                        self.at += 1;
                        self.next = if object {
                            Next::Key { closes: false }
                        } else {
                            Next::Value { closes: false }
                        };
                        return self.step();
                    }
                    (Some(b']'), false) => {
                        self.close();
                        Ok(Event::EndArray)
                    }
                    (Some(b'}'), true) => {
                        self.close();
                        Ok(Event::EndObject)
                    }
                    _ => Err(Malformed { at: self.at }),
                }
            }
        };
        match answer {
            Ok(event) => Some(Ok(event)),
            Err(malformed) => self.malformed(malformed.at),
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<Event<'a>, Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        self.step()
    }
}
