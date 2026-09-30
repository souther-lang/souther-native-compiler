//! What a Souther program calls that is not its own code.
//!
//! Room, and what a string is. A value made of fields has to live somewhere, and generated code
//! takes that room from here rather than deciding for itself where a value goes. Text is the first
//! thing here that is more than room: two strings are compared and joined by what they say and not
//! by where they are, so the answer has to be computed somewhere, and a loop emitted at every site
//! that compared two strings would be the same loop written as many times as a program says `==`.
//! Nothing a host implements is kept here: a behavior is called with the capabilities it was
//! constructed with, which whoever calls it builds and holds.
//!
//! Where a computation here turns out to be the one the wasm runtime already does — a calendar, a
//! regular expression — it is lifted into something both read. That is done when the second copy
//! exists and not before: until then there is nothing to tell a shared meaning from a shared
//! spelling. What the language says text means — its order, its length, its canonical form, what
//! each of the `String` module's kernels answers — is not written here: it is `souther_text`, over
//! bytes alone, and this reads the text out of a string, hands it over, and keeps what comes back
//! in the arena (`kernels`). A function of that kind added here instead would be a second copy of
//! what the wasm runtime is to read from there too (#17).

// Everything here is one half of a contract the other half reads by name, so an item whose doc has
// slid off it onto a neighbour is a contract nobody states. Refused rather than warned about.
#![deny(missing_docs)]

use souther_native_abi::{
    CARRIED, SLOT, TEXT_BYTES, TEXT_LENGTH, WHICH, room_for_carried, room_for_fields, room_for_text,
};

mod amount;
mod collection;
#[cfg(test)]
mod contract;
mod decimal;
mod decoding;
mod document;
mod enclosure;
mod external;
mod kernels;
mod magnitude;
mod rational;
mod temporal;
pub use collection::*;
pub use decimal::*;
pub use kernels::*;
pub use rational::*;
use souther_text::{Text as Held, append, code_points, compare};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};
pub use temporal::*;

/// How much room a run starts with, and how much more it takes each time it runs out.
///
/// Taken in one block and handed out by moving a mark, because a Souther value is never freed on
/// its own: what a run makes is dropped in one go by the caller that bracketed the call. So what a
/// value costs to make is a bounds check and an addition, and nothing here has to know what owns
/// what.
const BLOCK: usize = 1 << 17;

thread_local! {
    static ARENA: RefCell<Arena> = const { RefCell::new(Arena::new()) };
}

/// Room, in slots.
///
/// Blocks are slots and not bytes, so what a block is aligned to is what a slot is: the alignment
/// generated code is emitted against is one this provides rather than one the allocator happens to
/// give. Counted in slots for the same reason — a count of bytes would have to be rounded at every
/// place that read it, and one of them would eventually not be.
struct Arena {
    blocks: Vec<Vec<Slot>>,
    taken: usize,
}

/// One slot's worth of room. As wide as [`SLOT`] and aligned to it, which is what makes the whole
/// block aligned to it.
type Slot = u64;

impl Arena {
    const fn new() -> Self {
        Arena {
            blocks: Vec::new(),
            taken: 0,
        }
    }

    /// Room for `size` bytes, as slots.
    ///
    /// Blocks are never moved and never given back while a mark below them stands, so a pointer
    /// this answered stays where it is until that mark is reset. A vector of blocks rather than one
    /// growing block for that reason: growing one would move what a caller is holding.
    fn room(&mut self, size: usize) -> *mut u8 {
        // At least one, so that two values made of nothing are still two places. A value with no
        // fields is a value, and a pointer it shared with the next one would make them one.
        let wanted = size.div_ceil(SLOT as usize).max(1);
        let room = self.blocks.last().map_or(0, |it| it.capacity() - it.len());
        if room < wanted {
            self.blocks.push(Vec::with_capacity(wanted.max(BLOCK)));
        }
        let block = self.blocks.last_mut().expect("a block was just made");
        let at = block.len();
        block.resize(at + wanted, 0);
        self.taken += wanted;
        // Safe while the block is not reallocated, which `with_capacity` above is what keeps: the
        // resize never goes past the capacity the block was made with.
        unsafe { block.as_mut_ptr().add(at).cast() }
    }

    fn mark(&self) -> usize {
        self.taken
    }

    /// Drops everything taken since `mark`.
    ///
    /// Whole blocks go; a block a mark stands inside is cut back to where the mark is. What a
    /// caller holds from before the mark is untouched, which is the only thing this promises.
    fn reset(&mut self, mark: usize) {
        while self.taken > mark {
            let Some(block) = self.blocks.last_mut() else {
                self.taken = 0;
                return;
            };
            let held = block.len();
            if self.taken - held >= mark {
                self.taken -= held;
                self.blocks.pop();
            } else {
                let keep = held - (self.taken - mark);
                block.truncate(keep);
                self.taken = mark;
            }
        }
    }
}

/// Text of the layout `souther_native_abi` states, as the functions here take and answer it.
///
/// Nothing reads through this type: it is the address of a count of bytes and the text after it,
/// which the functions below read as bytes. It is a type of its own so that an address of text is
/// not an address of anything else where a function says what it takes, and what
/// `souther_native_abi` says each function takes is held to that (`host_table`).
#[repr(C)]
pub struct Text {
    _opaque: [u8; 0],
}

/// A list of the layout `souther_native_abi` states, as the functions here take and answer one: an
/// address the runtime reads through that layout alone, a type of its own for the reason [`Text`]
/// is.
#[repr(C)]
pub struct List {
    _opaque: [u8; 0],
}

/// A value of a declared type or of a union, as the functions here take and answer one: an address,
/// a type of its own for the reason [`Text`] is. The runtime reads behind one only where it made
/// what is there: a case no declaration names, carried with the token defined here
/// ([`souther_case_int_make`] and the rest).
#[repr(C)]
pub struct Value {
    _opaque: [u8; 0],
}

/// How many of something there are, or where one stands among them: bytes, issues, entries. The
/// same sixty-four bits as an `Int`, and not an `Int`, for the reason [`Text`] is a type.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Count(pub i64);

/// An open scope, as [`souther_scope_open`] answers it and [`souther_scope_close`] takes it back:
/// a token, and never where the arena stands.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope(pub i64);

/// Which of two strings comes first: below, at or above nought.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Comparison(pub i64);

/// Room for a value, reached by generated code.
///
/// # Safety
///
/// The pointer is good until the scope open around this call is closed. Reading it after that is
/// reading room something else has been handed.
/// # Panics
///
/// Where the size is below nought, which is generated code having worked one out wrongly rather
/// than a program doing anything. Read as nought it would answer a slot and the run would carry on
/// writing into room nobody asked for.
#[unsafe(no_mangle)]
pub extern "C" fn souther_alloc(size: Count) -> *mut u8 {
    let wanted =
        usize::try_from(size.0).expect("room is asked for in bytes, and never fewer than 0");
    ARENA.with(|it| it.borrow_mut().room(wanted))
}

/// Which ABI generation this runtime, and every library built with it, answers to: the one
/// function a host calls before any other, and the same in every generation
/// ([`souther_native_abi::GENERATION_QUERY`]).
#[unsafe(no_mangle)]
pub extern "C" fn souther_abi_generation() -> u32 {
    souther_native_abi::ABI_GENERATION
}

/// The next token a scope is answered with. Shared by every thread, so a token another thread was
/// answered is never one this thread has open.
static TOKENS: AtomicI64 = AtomicI64::new(1);

thread_local! {
    /// The scopes open on this thread, innermost last: each token and where the arena stood when
    /// it was opened.
    static SCOPES: RefCell<Vec<(i64, usize)>> = const { RefCell::new(Vec::new()) };
}

/// Opens a scope on this thread: what the library answers until it is closed lives until then.
#[unsafe(no_mangle)]
pub extern "C" fn souther_scope_open() -> Scope {
    let token = TOKENS.fetch_add(1, AtomicOrdering::Relaxed);
    let stood = ARENA.with(|it| it.borrow().mark());
    SCOPES.with(|it| it.borrow_mut().push((token, stood)));
    Scope(token)
}

/// Closes `scope` and drops everything made since it was opened, where it is the innermost scope
/// open on this thread, and answers whether it was. A token never answered, one already closed, one
/// with a scope still open inside it, or one another thread opened, is answered as that and
/// changes nothing: the host's mistake is its to report, and never a run's values dropped under
/// another scope still holding them.
#[unsafe(no_mangle)]
pub extern "C" fn souther_scope_close(scope: Scope) -> i8 {
    let stood = SCOPES.with(|it| {
        let mut open = it.borrow_mut();
        match open.last() {
            Some(&(token, stood)) if token == scope.0 => {
                open.pop();
                Some(stood)
            }
            _ => None,
        }
    });
    match stood {
        Some(stood) => {
            ARENA.with(|it| it.borrow_mut().reset(stood));
            1
        }
        None => 0,
    }
}

/// How many slots this thread's arena has handed out and not had back: an instrument for the
/// runtime's own tests and the driver's, which count what a run takes. In neither table, so no
/// header declares it and no library exports it.
#[unsafe(no_mangle)]
pub extern "C" fn souther_arena_taken() -> i64 {
    ARENA.with(|it| it.borrow().mark() as i64)
}

/// How many bytes of text a string carries.
///
/// # Safety
///
/// The pointer is one a string stands at: either room this arena answered and a string was written
/// into, or a string the object carries for a literal.
unsafe fn length(at: *const u8) -> usize {
    let said = unsafe { at.offset(TEXT_LENGTH as isize).cast::<i64>().read() };
    usize::try_from(said).expect("a string carries a count of bytes, and never fewer than 0")
}

/// The text a string holds, for as long as the pointer to it is borrowed.
///
/// Bound to a borrow of the caller's pointer and not to a lifetime the caller names: what a string
/// holds is good until the scope it was made in closes, which nothing here can see, so the text is let out
/// no further than the call that was handed the pointer.
///
/// # Safety
///
/// As [`length`]. And the string holds UTF-8, which is not asked again here: every string is
/// written by [`string_of`] from text, or is a literal the object carries, which the compiler writes
/// from text, and a host's text comes in through [`souther_string_of_utf8`], which refuses bytes
/// that are not.
pub(crate) unsafe fn text<'a, P>(at: &'a *const P) -> Held<'a> {
    let at = at.cast::<u8>();
    let bytes = unsafe { std::slice::from_raw_parts(at.offset(TEXT_BYTES as isize), length(at)) };
    Held::held(unsafe { std::str::from_utf8_unchecked(bytes) })
}

/// Room for a string of `bytes` bytes, with the count written and the text left to the caller.
///
/// How much that is comes from the crate that says where the text starts. Worked out here as a
/// slot and the bytes, it would be right only while the text happened to start one slot in — and
/// the day it did not, this would take less room than it then writes into.
fn room_for_a_string(bytes: usize) -> *mut u8 {
    let bytes = i64::try_from(bytes).expect("a string is smaller than an Int");
    let wanted = room_for_text(bytes);
    let at = souther_alloc(Count(wanted));
    unsafe { at.offset(TEXT_LENGTH as isize).cast::<i64>().write(bytes) };
    at
}

/// A string holding this text, in room the arena answered: the one way a string is written, so
/// that every string holds text.
pub(crate) fn string_of(text: &str) -> *mut Text {
    let at = room_for_a_string(text.len());
    unsafe {
        at.offset(TEXT_BYTES as isize)
            .copy_from_nonoverlapping(text.as_ptr(), text.len())
    };
    at.cast()
}

/// Two strings, in the order Souther gives text.
///
/// # Safety
///
/// Both pointers are ones a string stands at, and the scope each was made in is still open.
///
/// Every one of these is `unsafe` and not a safe function with a note about how to call it. What a
/// safe function promises is that no way of calling it from safe code is a memory fault, and these
/// read through what they are handed — so a caller reaching this crate as a Rust library, which it
/// is built as, could otherwise hand one of them anything at all and still be writing safe code.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_compare(
    left: *const Text,
    right: *const Text,
) -> Comparison {
    let ordering = unsafe { compare(text(&left), text(&right)) };
    Comparison(match ordering {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    })
}

/// A string's hash, from its bytes: two strings are equal where their bytes are
/// ([`souther_string_compare`]).
///
/// # Safety
///
/// As [`souther_string_compare`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_hash(at: *const Text) -> Hash {
    hash_of_bytes(unsafe { text(&at) }.as_str().as_bytes())
}

// The symbol that says which generation of the contract with generated code this runtime is
// (`souther_native_abi::runtime_generation_symbol`). Written by the build from the abi crate's
// number, so that an object of another generation has an undefined symbol and not a call answered
// the other way.
include!(concat!(env!("OUT_DIR"), "/runtime_generation.rs"));

/// What a string holds: how much text, in Unicode code points.
///
/// The bound is the language's and not this carrier's own (spec §what-a-string-holds, ADR-0096):
/// every carrier holds the same `souther_text::LONGEST_TEXT`, so whether a text has a place does
/// not depend on which carrier answers. It is handed to every operation that builds text
/// (`souther_text`), which spends it as it builds and writes nothing past it.
pub(crate) const STRING_HOLDS: souther_text::Capacity =
    souther_text::Capacity::of_code_points(souther_text::LONGEST_TEXT);

/// The two strings' text, one after the other, as a string of its own: `++` over two strings, and
/// `String.append`.
///
/// In NFC, which each of the two is and the join need not be: a letter ending the one and a mark
/// beginning the other compose into one code point (spec §string-canonical).
///
/// Neither operand is touched. A Souther value is immutable and nothing frees one on its own, so
/// joining two of them is a third value and never a longer first one.
///
/// # Safety
///
/// As [`souther_string_compare`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_concat(
    left: *const Text,
    right: *const Text,
    out: *mut *mut Text,
) -> i8 {
    let joined = unsafe { append(text(&left), text(&right), STRING_HOLDS) };
    unsafe { answered(joined.as_deref().map(string_of), out) }
}

/// A string holding this text, for a caller outside a Souther program.
///
/// What generated code makes a string from is a literal the object carries or a join of two it
/// already holds. This is the other direction — a host handing text in — and it is here rather
/// than written by each such host so that the layout stays between this crate and the one that
/// states it.
///
/// A door, as a decoder's string leaf is: text arriving from outside is admitted here, put in NFC
/// by the language's Unicode version, whatever the host's own is, and within what a `String` holds
/// (`souther_text::admitted`). So every string holds text as the language says a string is,
/// whoever made it. `out` is written and `1` answered where it is; nothing is written and `0` is
/// answered where there is no `String` of the bytes: a count below nought, bytes that are not
/// UTF-8, or text whose canonical value has no place a `String` holds. The runtime decides all
/// three and ends nothing on any of them; a binding that wants to tell them apart, to say which a
/// host's text was, checks the first two itself, as each of this repository's does, and turns the
/// third into whatever it calls `RequiredFormHasNoPlace`.
///
/// # Safety
///
/// `bytes` points at `length` bytes that may be read, where `length` is above nought, and `out` is
/// room for the address of a `Text`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_of_utf8(
    bytes: *const u8,
    length: Count,
    out: *mut *mut Text,
) -> i8 {
    let Ok(held) = usize::try_from(length.0) else {
        return unsafe { answered(None, out) };
    };
    let bytes = if held == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, held) }
    };
    let admitted = souther_text::admitted(bytes, STRING_HOLDS).ok();
    unsafe { answered(admitted.as_deref().map(string_of), out) }
}

/// How many bytes of text the string carries, for the same caller.
///
/// # Safety
///
/// As [`souther_string_compare`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_length(at: *const Text) -> Count {
    Count(unsafe { length(at.cast()) as i64 })
}

/// Where that text stands.
///
/// # Safety
///
/// As [`souther_string_compare`]. What is answered is good for as long as the string is.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_bytes(at: *const Text) -> *const u8 {
    unsafe { at.cast::<u8>().offset(TEXT_BYTES as isize) }
}

/// How long the string is as the language counts it: in code points, which is what
/// `String.length` answers.
///
/// Counted over the bytes each time rather than held beside them. The layout keeps one count, of
/// bytes, and a second would be one every place that makes a string has to keep right. What is
/// counted is `souther_text`'s, which the order below reads text through too.
///
/// # Safety
///
/// As [`souther_string_compare`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_code_points(at: *const Text) -> i64 {
    let counted = code_points(unsafe { text(&at) });
    i64::try_from(counted).expect("a string holds fewer code points than an Int counts")
}

/// The token of every case in `souther_native_abi::BUILT_IN_CASES`, defined under the symbol
/// `built_in_case_symbol` spells for it. What each is for is its address; the byte is what gives
/// each one an address of its own, as `TOKEN` says of a declared type's.
///
/// The symbol and the entry in [`BUILT_IN_CASE_TOKENS`] are made from the one name, so the table
/// says what is defined and a test holds it to the one `souther_native_abi` states.
macro_rules! built_in_cases {
    ($($name:literal => $item:ident),* $(,)?) => {
        $(
            #[doc = concat!("The token a value of the case `", $name, "` carries.")]
            #[unsafe(export_name = concat!("souther$case$", $name))]
            pub static $item: [u8; 1] = [0];
        )*

        /// Every token defined here, by the name of its case.
        #[cfg(test)]
        const BUILT_IN_CASE_TOKENS: &[(&str, &[u8; 1])] = &[$(($name, &$item)),*];
    };
}

built_in_cases! {
    "Int" => CASE_INT,
    "Bool" => CASE_BOOL,
    "String" => CASE_STRING,
    "Decimal" => CASE_DECIMAL,
    "Rational" => CASE_RATIONAL,
    "Date" => CASE_DATE,
    "Time" => CASE_TIME,
    "DateTime" => CASE_DATETIME,
    "Instant" => CASE_INSTANT,
    "Some" => CASE_SOME,
    "None" => CASE_NONE,
    "DivisionByZero" => CASE_DIVISION_BY_ZERO,
    "NotANumber" => CASE_NOT_A_NUMBER,
    "NotADate" => CASE_NOT_A_DATE,
    "NotATime" => CASE_NOT_A_TIME,
    "NotWhole" => CASE_NOT_WHOLE,
    "NotAFiniteDecimal" => CASE_NOT_A_FINITE_DECIMAL,
}

/// A value of a case no declaration names, as a union holds one: room with the case's token at
/// `WHICH`, and what the case holds, where it holds something, at `CARRIED`. Laid out as generated
/// code lays one out when it carries a primitive or makes a case the language gives, so a value a
/// host made here and one a run made are one representation.
fn carried(token: &[u8; 1], held: Option<i64>) -> *const Value {
    let size = match held {
        Some(_) => room_for_carried(),
        None => room_for_fields(0),
    };
    let room = souther_alloc(Count(size));
    // SAFETY: the room was just taken, at least as wide as the slots written, and aligned to a
    // slot as every room the arena hands out is.
    unsafe {
        room.add(WHICH as usize)
            .cast::<i64>()
            .write(token.as_ptr() as i64);
        if let Some(held) = held {
            room.add(CARRIED as usize).cast::<i64>().write(held);
        }
    }
    room.cast_const().cast()
}

/// What a value of a union carrying the primitive whose case `token` is holds, or none where the
/// value is another case: which case a value is, is read off the value, so a host asking a value
/// for a case it is not is answered that and not handed what another case's slot holds.
///
/// # Safety
///
/// `value` is a value of a union the library answered, and the scope it was made in is open.
unsafe fn held(value: *const Value, token: &[u8; 1]) -> Option<i64> {
    unsafe {
        let at = value.cast::<u8>();
        let which = at.add(WHICH as usize).cast::<i64>().read();
        (which == token.as_ptr() as i64).then(|| at.add(CARRIED as usize).cast::<i64>().read())
    }
}

/// An `Int` carried as a case of a union.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_int_make(value: i64) -> *const Value {
    carried(&CASE_INT, Some(value))
}

/// What a value of a union that is the case `Int` holds, written through `out` where it is
/// that case, answering whether it is.
///
/// # Safety
///
/// `value` is a value of a union the library answered, the scope it was made in is open, and
/// `out` is room for what the case holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_case_int_read(value: *const Value, out: *mut i64) -> i8 {
    unsafe { answered(held(value, &CASE_INT), out) }
}

/// A `Bool` carried as a case of a union: nought or one in its slot, as generated code widens one.
/// A host's byte is a `Bool` by being nought or not, so any other byte is `true`, and the slot
/// never holds a third value.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_bool_make(value: i8) -> *const Value {
    carried(&CASE_BOOL, Some(i64::from(value != 0)))
}

/// What a value of a union that is the case `Bool` holds, written through `out` where it is
/// that case, answering whether it is.
///
/// # Safety
///
/// `value` is a value of a union the library answered, the scope it was made in is open, and
/// `out` is room for what the case holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_case_bool_read(value: *const Value, out: *mut i8) -> i8 {
    unsafe { answered(held(value, &CASE_BOOL).map(|it| i8::from(it != 0)), out) }
}

/// A `String` carried as a case of a union: its address in the slot, the text where it was.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_string_make(value: *const Text) -> *const Value {
    carried(&CASE_STRING, Some(value as i64))
}

/// What a value of a union that is the case `String` holds, written through `out` where it is
/// that case, answering whether it is.
///
/// # Safety
///
/// `value` is a value of a union the library answered, the scope it was made in is open, and
/// `out` is room for what the case holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_case_string_read(value: *const Value, out: *mut *mut Text) -> i8 {
    unsafe { answered(held(value, &CASE_STRING).map(|it| it as *mut Text), out) }
}

/// A `Decimal` carried as a case of a union: its address in the slot, the value where it was.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_decimal_make(value: *const Decimal) -> *const Value {
    carried(&CASE_DECIMAL, Some(value as i64))
}

/// What a value of a union that is the case `Decimal` holds, written through `out` where it is
/// that case, answering whether it is.
///
/// # Safety
///
/// `value` is a value of a union the library answered, the scope it was made in is open, and
/// `out` is room for what the case holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_case_decimal_read(
    value: *const Value,
    out: *mut *mut Decimal,
) -> i8 {
    unsafe { answered(held(value, &CASE_DECIMAL).map(|it| it as *mut Decimal), out) }
}

/// A `Date` carried as a case of a union: its address in the slot, the value where it was.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_date_make(value: *const Date) -> *const Value {
    carried(&CASE_DATE, Some(value as i64))
}

/// What a value of a union that is the case `Date` holds, written through `out` where it is
/// that case, answering whether it is.
///
/// # Safety
///
/// `value` is a value of a union the library answered, the scope it was made in is open, and
/// `out` is room for what the case holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_case_date_read(value: *const Value, out: *mut *mut Date) -> i8 {
    unsafe { answered(held(value, &CASE_DATE).map(|it| it as *mut Date), out) }
}

/// A `Time` carried as a case of a union: its address in the slot, the value where it was.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_time_make(value: *const Time) -> *const Value {
    carried(&CASE_TIME, Some(value as i64))
}

/// What a value of a union that is the case `Time` holds, written through `out` where it is
/// that case, answering whether it is.
///
/// # Safety
///
/// `value` is a value of a union the library answered, the scope it was made in is open, and
/// `out` is room for what the case holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_case_time_read(value: *const Value, out: *mut *mut Time) -> i8 {
    unsafe { answered(held(value, &CASE_TIME).map(|it| it as *mut Time), out) }
}

/// A `DateTime` carried as a case of a union: its address in the slot, the value where it was.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_datetime_make(value: *const DateTime) -> *const Value {
    carried(&CASE_DATETIME, Some(value as i64))
}

/// What a value of a union that is the case `DateTime` holds, written through `out` where it is
/// that case, answering whether it is.
///
/// # Safety
///
/// `value` is a value of a union the library answered, the scope it was made in is open, and
/// `out` is room for what the case holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_case_datetime_read(
    value: *const Value,
    out: *mut *mut DateTime,
) -> i8 {
    unsafe {
        answered(
            held(value, &CASE_DATETIME).map(|it| it as *mut DateTime),
            out,
        )
    }
}

/// A `Instant` carried as a case of a union: its address in the slot, the value where it was.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_instant_make(value: *const Instant) -> *const Value {
    carried(&CASE_INSTANT, Some(value as i64))
}

/// What a value of a union that is the case `Instant` holds, written through `out` where it is
/// that case, answering whether it is.
///
/// # Safety
///
/// `value` is a value of a union the library answered, the scope it was made in is open, and
/// `out` is room for what the case holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_case_instant_read(
    value: *const Value,
    out: *mut *mut Instant,
) -> i8 {
    unsafe { answered(held(value, &CASE_INSTANT).map(|it| it as *mut Instant), out) }
}

// Each case the language gives holds nothing, so a value of it is its token alone. Written out one
// by one, so that the contract's scan of this source finds every function a host is told of.

/// A value of the case `Some`, as a union holds one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_some_make() -> *const Value {
    carried(&CASE_SOME, None)
}

/// A value of the case `None`, as a union holds one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_none_make() -> *const Value {
    carried(&CASE_NONE, None)
}

/// A value of the case `DivisionByZero`, as a union holds one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_division_by_zero_make() -> *const Value {
    carried(&CASE_DIVISION_BY_ZERO, None)
}

/// A value of the case `NotANumber`, as a union holds one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_not_a_number_make() -> *const Value {
    carried(&CASE_NOT_A_NUMBER, None)
}

/// A value of the case `NotADate`, as a union holds one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_not_a_date_make() -> *const Value {
    carried(&CASE_NOT_A_DATE, None)
}

/// A value of the case `NotATime`, as a union holds one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_not_a_time_make() -> *const Value {
    carried(&CASE_NOT_A_TIME, None)
}

/// A value of the case `NotWhole`, as a union holds one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_not_whole_make() -> *const Value {
    carried(&CASE_NOT_WHOLE, None)
}

/// A value of the case `NotAFiniteDecimal`, as a union holds one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_case_not_a_finite_decimal_make() -> *const Value {
    carried(&CASE_NOT_A_FINITE_DECIMAL, None)
}

#[cfg(test)]
mod tests {
    use super::{
        BUILT_IN_CASE_TOKENS, Count, Scope, Text, souther_alloc, souther_arena_taken,
        souther_scope_close, souther_scope_open, souther_string_bytes, souther_string_code_points,
        souther_string_compare, souther_string_concat, souther_string_length,
        souther_string_of_utf8,
    };

    use souther_native_abi::{BUILT_IN_CASES, SLOT};
    use std::collections::BTreeSet;

    /// A string holding this text, as a host outside a Souther program would hand one over.
    ///
    /// What the call owes is said here and not at every row below: the bytes are a `str`'s, so
    /// there are as many of them as this says and they are valid UTF-8; and every string these
    /// tests make is given back before the scope they were made in closes.
    fn made(text: &str) -> *mut Text {
        let mut out = std::ptr::null_mut();
        let admitted =
            unsafe { souther_string_of_utf8(text.as_ptr(), Count(text.len() as i64), &mut out) };
        assert_eq!(admitted, 1, "test text has a place");
        out
    }

    /// The two compared, and the two joined, under what [`made`] already owes.
    fn compared(one: *const Text, other: *const Text) -> i64 {
        unsafe { souther_string_compare(one, other) }.0
    }

    fn joined_text(one: *const Text, other: *const Text) -> *mut Text {
        let mut out = std::ptr::null_mut();
        assert_eq!(unsafe { souther_string_concat(one, other, &mut out) }, 1);
        out
    }

    /// Room for `size` bytes, as generated code asks for it.
    fn room(size: i64) -> *mut u8 {
        souther_alloc(Count(size))
    }

    /// What the string says, read back the way a host reads one.
    fn said(at: *const Text) -> String {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                souther_string_bytes(at),
                souther_string_length(at).0 as usize,
            )
        };
        String::from_utf8(bytes.to_vec()).expect("a string carries the text it was made from")
    }

    /// A length is counted in code points, so a character past the basic plane is one, and a flag
    /// is the two regional indicators it is made of and not the one thing a reader sees.
    #[test]
    fn a_length_counts_code_points_and_not_bytes_or_what_a_reader_sees() {
        let scope = souther_scope_open();
        for (text, counted) in [
            ("", 0),
            ("cart", 4),
            ("é", 1),
            ("日本語", 3),
            ("𠮷", 1),
            ("🇯🇵", 2),
            ("a𠮷b", 3),
        ] {
            let at = made(text);
            assert_eq!(unsafe { souther_string_code_points(at) }, counted, "{text}");
        }
        souther_scope_close(scope);
    }

    /// Every case the table names has a token here, and nothing else does, and each token is a
    /// place of its own: a token defined and not tabled, or tabled and not defined, is a case one
    /// side tags a value with and the other never names.
    #[test]
    fn every_built_in_case_has_a_token_of_its_own() {
        let defined: BTreeSet<&str> = BUILT_IN_CASE_TOKENS.iter().map(|(name, _)| *name).collect();
        let tabled: BTreeSet<&str> = BUILT_IN_CASES.iter().copied().collect();
        assert_eq!(defined, tabled);
        assert_eq!(BUILT_IN_CASE_TOKENS.len(), BUILT_IN_CASES.len());

        let places: BTreeSet<*const u8> = BUILT_IN_CASE_TOKENS
            .iter()
            .map(|(_, token)| token.as_ptr())
            .collect();
        assert_eq!(places.len(), BUILT_IN_CASES.len());
    }

    #[test]
    fn room_answered_twice_is_two_different_places() {
        let one = room(8);
        let other = room(8);
        assert_ne!(one, other);
    }

    /// What generated code is emitted against. Every access it makes says the address is aligned,
    /// and a misaligned one is allowed to answer wrongly rather than to fail — so nothing would
    /// report this going wrong except the answers.
    #[test]
    fn every_pointer_answered_is_aligned_to_a_slot() {
        let scope = souther_scope_open();
        for size in [1i64, 7, 8, 9, 16, 40, 4096] {
            for _ in 0..4 {
                let at = room(size);
                assert_eq!(
                    at as usize % SLOT as usize,
                    0,
                    "room for {size} bytes answered {at:?}"
                );
            }
        }
        souther_scope_close(scope);
    }

    /// Room for less than a slot is still a slot, so what is written into it does not reach into
    /// what was answered next.
    #[test]
    fn room_for_less_than_a_slot_is_a_slot_of_its_own() {
        let scope = souther_scope_open();
        let one = room(1).cast::<i64>();
        let other = room(1).cast::<i64>();
        unsafe {
            one.write(-1);
            other.write(0);
            assert_eq!(one.read(), -1);
        }
        souther_scope_close(scope);
    }

    #[test]
    fn what_was_written_is_there_until_the_mark_is_reset() {
        let scope = souther_scope_open();
        let held = room(16).cast::<i64>();
        unsafe {
            held.write(7);
            held.add(1).write(11);
            assert_eq!(held.read(), 7);
            assert_eq!(held.add(1).read(), 11);
        }
        souther_scope_close(scope);
    }

    /// What the bracket is for: a run takes room, the caller gives it back, and the next run
    /// starts where the first one did rather than further along.
    ///
    /// Where it starts and not which address it is handed. Giving room back may hand a whole block
    /// to the allocator, and what comes back next is wherever that allocator answers; what this
    /// promises is that a run repeated a thousand times costs what one costs.
    #[test]
    fn room_taken_in_a_scope_is_taken_again_rather_than_added_to() {
        let before = souther_arena_taken();
        let scope = souther_scope_open();
        let _taken = room(32);
        let after_one = souther_arena_taken();
        assert_eq!(souther_scope_close(scope), 1);

        for _ in 0..1000 {
            let scope = souther_scope_open();
            let _taken = room(32);
            assert_eq!(souther_arena_taken(), after_one);
            assert_eq!(souther_scope_close(scope), 1);
        }
        assert_eq!(souther_arena_taken(), before);
    }

    /// A scope closes only as the innermost open scope of the thread that opened it, and once: any
    /// other token is answered as that and drops nothing.
    #[test]
    fn a_scope_that_is_not_the_innermost_open_one_is_refused_and_drops_nothing() {
        let outer = souther_scope_open();
        let inner = souther_scope_open();
        let held = room(8).cast::<i64>();
        unsafe { held.write(7) };
        assert_eq!(souther_scope_close(outer), 0, "out of order");
        assert_eq!(souther_scope_close(Scope(i64::MAX)), 0, "never answered");
        assert_eq!(unsafe { held.read() }, 7);
        let elsewhere = std::thread::spawn(|| souther_scope_open()).join().unwrap();
        assert_eq!(souther_scope_close(elsewhere), 0, "another thread's");
        assert_eq!(souther_scope_close(inner), 1);
        assert_eq!(souther_scope_close(inner), 0, "twice");
        assert_eq!(souther_scope_close(outer), 1);
    }

    /// Closing a scope takes back what was made in it and no more.
    #[test]
    fn what_was_taken_before_a_mark_stays_where_it_is() {
        let held = room(8).cast::<i64>();
        unsafe { held.write(42) };
        let scope = souther_scope_open();
        let _dropped = room(4096);
        souther_scope_close(scope);
        assert_eq!(unsafe { held.read() }, 42);
    }

    /// A block's worth at a time, so what is answered crosses the block the arena starts with.
    #[test]
    fn room_past_one_block_is_still_room() {
        let scope = souther_scope_open();
        let mut held = Vec::new();
        for value in 0..4096i64 {
            let at = room(1024).cast::<i64>();
            unsafe { at.write(value) };
            held.push(at);
        }
        for (value, at) in held.iter().enumerate() {
            assert_eq!(unsafe { at.read() }, value as i64);
        }
        souther_scope_close(scope);
    }

    /// A host's text is admitted where it comes in: put in NFC by the language's Unicode version,
    /// so a host normalizing by its own, or not at all, hands over the same string.
    #[test]
    fn a_hosts_text_is_put_in_nfc_where_it_comes_in() {
        let scope = souther_scope_open();
        assert_eq!(said(made("e\u{301}")), "\u{e9}");
        assert_eq!(compared(made("e\u{301}"), made("\u{e9}")), 0);
        souther_scope_close(scope);
    }

    #[test]
    fn a_string_carries_the_text_it_was_made_from() {
        let scope = souther_scope_open();
        assert_eq!(said(made("hello")), "hello");
        assert_eq!(said(made("")), "");
        assert_eq!(said(made("\u{0}after a nought")), "\u{0}after a nought");
        souther_scope_close(scope);
    }

    /// Text a host hands over past what a `String` holds (`souther_text::LONGEST_TEXT`, 2^28 - 1
    /// code points) has no place: `souther_string_of_utf8` answers `0` and writes nothing through
    /// `out`, the same `Bool` + room shape a generated string operation already answers by
    /// (souther-native-compiler#109).
    ///
    /// This is the one place the real, production `STRING_HOLDS` is exercised rather than a small
    /// stand-in — everywhere else that would ask for text this long asks `souther_text::admitted`
    /// directly at a capacity of its own choosing, which is what pins the boundary arithmetic
    /// itself (see `souther-text`'s own tests). What is worth pinning here, once, is only that this
    /// FFI door is actually wired to the real constant and answers the `Bool` + room shape at that
    /// scale — so this asks for the refusal alone, not also a success at exactly the bound, which
    /// would cost a second quarter-gigabyte allocation to prove something every other test in this
    /// module already exercises at ordinary sizes. ASCII, so code points, UTF-16 units and bytes
    /// coincide and the allocation stays a plain memset.
    #[test]
    fn text_past_what_a_string_holds_has_no_place() {
        let scope = souther_scope_open();
        let longest = usize::try_from(souther_text::LONGEST_TEXT).unwrap();
        let past_the_bound = "a".repeat(longest + 1);
        let mut out = std::ptr::null_mut();
        let admitted = unsafe {
            souther_string_of_utf8(
                past_the_bound.as_ptr(),
                Count(past_the_bound.len() as i64),
                &mut out,
            )
        };
        assert_eq!(admitted, 0, "one code point more has no place");
        assert!(out.is_null(), "nothing is written where there is no place");
        souther_scope_close(scope);
    }

    /// Two strings are equal by what they say. Made separately they stand at two addresses, and an
    /// answer read off the addresses would be the wrong one for exactly this pair.
    #[test]
    fn two_strings_of_one_text_made_separately_are_equal() {
        let scope = souther_scope_open();
        let one = made("hello");
        let other = made("hello");

        assert_ne!(one, other);
        assert_eq!(compared(one, other), 0);
        souther_scope_close(scope);
    }

    #[test]
    fn a_string_that_begins_another_comes_before_it() {
        let scope = souther_scope_open();
        assert_eq!(compared(made("ab"), made("abc")), -1);
        assert_eq!(compared(made("abc"), made("ab")), 1);
        assert_eq!(compared(made(""), made("a")), -1);
        souther_scope_close(scope);
    }

    /// The order is by scalar value, which is the order of the bytes and not the order of a JVM
    /// string's UTF-16 code units.
    ///
    /// `𠮷` is U+20BB7 and `￥` is U+FFE5. By scalar value, and by the UTF-8 bytes, the first is the
    /// greater. As UTF-16 the first begins D842, which is below FFE5, so an implementation that
    /// read the text back as those units would answer the other way here.
    #[test]
    fn text_is_ordered_by_scalar_value_and_not_by_utf_16_code_unit() {
        let scope = souther_scope_open();
        let astral = "\u{20bb7}";
        let basic = "\u{ffe5}";

        assert_eq!(compared(made(astral), made(basic)), 1);
        assert_eq!(compared(made(basic), made(astral)), -1);
        assert!(astral.encode_utf16().next() < basic.encode_utf16().next());
        souther_scope_close(scope);
    }

    #[test]
    fn two_strings_joined_say_one_and_then_the_other() {
        let scope = souther_scope_open();
        assert_eq!(said(joined_text(made("ab"), made("cd"))), "abcd");
        assert_eq!(said(joined_text(made(""), made("cd"))), "cd");
        assert_eq!(said(joined_text(made("ab"), made(""))), "ab");
        souther_scope_close(scope);
    }

    /// A join makes a third value and leaves both operands as they were. Nothing frees a Souther
    /// value on its own and none of them is modified, so what a caller held before the join it
    /// still holds after it.
    #[test]
    fn a_join_leaves_both_of_its_operands_as_they_were() {
        let scope = souther_scope_open();
        let before = made("ab");
        let after = made("cd");

        let joined = joined_text(before, after);

        assert_eq!(said(joined), "abcd");
        assert_eq!(said(before), "ab");
        assert_eq!(said(after), "cd");
        souther_scope_close(scope);
    }

    /// A join is a string like any other, so joining one again is not a different question.
    #[test]
    fn a_joined_string_is_one_that_can_be_joined_and_compared_again() {
        let scope = souther_scope_open();
        let joined = joined_text(made("ab"), made("cd"));

        assert_eq!(compared(joined, made("abcd")), 0);
        assert_eq!(said(joined_text(joined, made("ef"))), "abcdef");
        souther_scope_close(scope);
    }
}
