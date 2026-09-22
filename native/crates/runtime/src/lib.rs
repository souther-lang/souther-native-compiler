//! What a Souther program calls that is not its own code.
//!
//! Room, and what a string is. A value made of fields has to live somewhere, and generated code
//! takes that room from here rather than deciding for itself where a value goes. Text is the first
//! thing here that is more than room: two strings are compared and joined by what they say and not
//! by where they are, so the answer has to be computed somewhere, and a loop emitted at every site
//! that compared two strings would be the same loop written as many times as a program says `==`.
//!
//! Where a computation here turns out to be the one the wasm runtime already does — a calendar, a
//! regular expression — it is lifted into something both read. That is done when the second copy
//! exists and not before: until then there is nothing to tell a shared meaning from a shared
//! spelling.

use souther_native_abi::{SLOT, TEXT_BYTES, TEXT_LENGTH};
use std::cell::RefCell;
use std::cmp::Ordering;

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

/// Room for a value, reached by generated code.
///
/// # Safety
///
/// The pointer is good until a mark taken before this call is reset. Reading it after that is
/// reading room something else has been handed.
/// # Panics
///
/// Where the size is below nought, which is generated code having worked one out wrongly rather
/// than a program doing anything. Read as nought it would answer a slot and the run would carry on
/// writing into room nobody asked for.
#[unsafe(no_mangle)]
pub extern "C" fn souther_alloc(size: i64) -> *mut u8 {
    let wanted = usize::try_from(size).expect("room is asked for in bytes, and never fewer than 0");
    ARENA.with(|it| it.borrow_mut().room(wanted))
}

/// Where the arena stands, for a caller about to bracket a call.
#[unsafe(no_mangle)]
pub extern "C" fn souther_mark() -> i64 {
    ARENA.with(|it| it.borrow().mark() as i64)
}

/// Drops what a call made, back to `mark`.
/// # Panics
///
/// Where the mark is one this never issued. Read as nought it would drop what a caller further out
/// is still holding, which is the one thing a mark is for.
#[unsafe(no_mangle)]
pub extern "C" fn souther_reset(mark: i64) {
    let held = usize::try_from(mark).expect("a mark is one this arena answered");
    ARENA.with(|it| it.borrow_mut().reset(held));
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

/// The text a string carries.
///
/// # Safety
///
/// As [`length`], and for as long as the mark below the string stands.
unsafe fn text<'a>(at: *const u8) -> &'a [u8] {
    unsafe { std::slice::from_raw_parts(at.offset(TEXT_BYTES as isize), length(at)) }
}

/// Room for a string of `bytes` bytes, with the count written and the text left to the caller.
fn room_for_text(bytes: usize) -> *mut u8 {
    let wanted = i64::try_from(SLOT as usize + bytes).expect("a string is smaller than an Int");
    let at = souther_alloc(wanted);
    unsafe {
        at.offset(TEXT_LENGTH as isize)
            .cast::<i64>()
            .write(bytes as i64)
    };
    at
}

/// Two strings, in the order Souther gives text.
///
/// # Safety
///
/// Both pointers are ones a string stands at.
#[unsafe(no_mangle)]
pub extern "C" fn souther_string_compare(left: *const u8, right: *const u8) -> i64 {
    let ordering = unsafe { compare_utf8_as_utf16(text(left), text(right)) };
    match ordering {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// The two strings' text, one after the other, as a string of its own.
///
/// Neither operand is touched. A Souther value is immutable and nothing frees one on its own, so
/// joining two of them is a third value and never a longer first one.
///
/// # Safety
///
/// As [`souther_string_compare`].
#[unsafe(no_mangle)]
pub extern "C" fn souther_string_concat(left: *const u8, right: *const u8) -> *mut u8 {
    let (before, after) = unsafe { (text(left), text(right)) };
    let at = room_for_text(before.len() + after.len());
    unsafe {
        let text = at.offset(TEXT_BYTES as isize);
        text.copy_from_nonoverlapping(before.as_ptr(), before.len());
        text.add(before.len())
            .copy_from_nonoverlapping(after.as_ptr(), after.len());
    }
    at
}

/// A string holding these bytes, for a caller outside a Souther program.
///
/// What generated code makes a string from is a literal the object carries or a join of two it
/// already holds. This is the other direction — a host handing text in — and it is here rather
/// than written by each such host so that the layout stays between this crate and the one that
/// states it.
///
/// # Safety
///
/// `bytes` points at `length` bytes.
/// # Panics
///
/// Where the length is below nought.
#[unsafe(no_mangle)]
pub extern "C" fn souther_string_of_utf8(bytes: *const u8, length: i64) -> *mut u8 {
    let held = usize::try_from(length).expect("text is handed over as bytes, and never fewer than 0");
    let at = room_for_text(held);
    unsafe { at.offset(TEXT_BYTES as isize).copy_from_nonoverlapping(bytes, held) };
    at
}

/// How many bytes of text the string carries, for the same caller.
///
/// # Safety
///
/// As [`souther_string_compare`].
#[unsafe(no_mangle)]
pub extern "C" fn souther_string_length(at: *const u8) -> i64 {
    unsafe { length(at) as i64 }
}

/// Where that text stands.
///
/// # Safety
///
/// As [`souther_string_compare`]. What is answered is good for as long as the string is.
#[unsafe(no_mangle)]
pub extern "C" fn souther_string_bytes(at: *const u8) -> *const u8 {
    unsafe { at.offset(TEXT_BYTES as isize) }
}

/// Two runs of text, compared by UTF-16 code unit.
///
/// Which is not the order the bytes are in, and not the order the code points are in either. A code
/// point past the basic plane is two units beginning at D800, and a unit from E000 up is one — so
/// `𠮷` (U+20BB7) comes before `￥` (U+FFE5) here and after it by either of the other two readings.
/// That is what Souther's `<` over two strings answers, and it is what the JVM answers, which is
/// what a row recorded before a native run was ever put to it.
///
/// Said as a run of bytes and nothing else, so that the day the wasm runtime's copy of this and
/// this one are the same algorithm, what moves is a function over two slices — no arena, no
/// address of either carrier's width, and nothing about where a string is kept.
fn compare_utf8_as_utf16(left: &[u8], right: &[u8]) -> Ordering {
    let mut a = Units::over(left);
    let mut b = Units::over(right);
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x != y => return x.cmp(&y),
            _ => {}
        }
    }
}

/// The UTF-16 code units a run of UTF-8 spells, one at a time.
///
/// A pair is answered over two turns, which is what `pending` holds: the second unit of a surrogate
/// pair is never nought, so nought stands for there being none.
struct Units<'a> {
    text: &'a [u8],
    at: usize,
    pending: u16,
}

impl<'a> Units<'a> {
    fn over(text: &'a [u8]) -> Units<'a> {
        Units {
            text,
            at: 0,
            pending: 0,
        }
    }

    fn next(&mut self) -> Option<u16> {
        if self.pending != 0 {
            let low = self.pending;
            self.pending = 0;
            return Some(low);
        }
        let first = u32::from(*self.text.get(self.at)?);
        let (point, width) = if first < 0x80 {
            (first, 1)
        } else if first < 0xe0 {
            (((first & 0x1f) << 6) | self.trailing(1), 2)
        } else if first < 0xf0 {
            (
                ((first & 0x0f) << 12) | (self.trailing(1) << 6) | self.trailing(2),
                3,
            )
        } else {
            (
                ((first & 0x07) << 18)
                    | (self.trailing(1) << 12)
                    | (self.trailing(2) << 6)
                    | self.trailing(3),
                4,
            )
        };
        self.at += width;
        if point > 0xffff {
            let rest = point - 0x10000;
            self.pending = 0xdc00 + (rest & 0x3ff) as u16;
            Some(0xd800 + (rest >> 10) as u16)
        } else {
            Some(point as u16)
        }
    }

    fn trailing(&self, offset: usize) -> u32 {
        u32::from(self.text.get(self.at + offset).copied().unwrap_or(0)) & 0x3f
    }
}

#[cfg(test)]
mod tests {
    use super::{
        souther_alloc, souther_mark, souther_reset, souther_string_bytes, souther_string_compare,
        souther_string_concat, souther_string_length, souther_string_of_utf8,
    };

    use souther_native_abi::SLOT;

    /// A string holding this text, as a host outside a Souther program would hand one over.
    fn made(text: &str) -> *mut u8 {
        souther_string_of_utf8(text.as_ptr(), text.len() as i64)
    }

    /// What the string says, read back the way a host reads one.
    fn said(at: *const u8) -> String {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                souther_string_bytes(at),
                souther_string_length(at) as usize,
            )
        };
        String::from_utf8(bytes.to_vec()).expect("a string carries the text it was made from")
    }

    #[test]
    fn room_answered_twice_is_two_different_places() {
        let one = souther_alloc(8);
        let other = souther_alloc(8);
        assert_ne!(one, other);
    }

    /// What generated code is emitted against. Every access it makes says the address is aligned,
    /// and a misaligned one is allowed to answer wrongly rather than to fail — so nothing would
    /// report this going wrong except the answers.
    #[test]
    fn every_pointer_answered_is_aligned_to_a_slot() {
        let mark = souther_mark();
        for size in [1i64, 7, 8, 9, 16, 40, 4096] {
            for _ in 0..4 {
                let at = souther_alloc(size);
                assert_eq!(
                    at as usize % SLOT as usize,
                    0,
                    "room for {size} bytes answered {at:?}"
                );
            }
        }
        souther_reset(mark);
    }

    /// Room for less than a slot is still a slot, so what is written into it does not reach into
    /// what was answered next.
    #[test]
    fn room_for_less_than_a_slot_is_a_slot_of_its_own() {
        let mark = souther_mark();
        let one = souther_alloc(1).cast::<i64>();
        let other = souther_alloc(1).cast::<i64>();
        unsafe {
            one.write(-1);
            other.write(0);
            assert_eq!(one.read(), -1);
        }
        souther_reset(mark);
    }

    #[test]
    fn what_was_written_is_there_until_the_mark_is_reset() {
        let mark = souther_mark();
        let held = souther_alloc(16).cast::<i64>();
        unsafe {
            held.write(7);
            held.add(1).write(11);
            assert_eq!(held.read(), 7);
            assert_eq!(held.add(1).read(), 11);
        }
        souther_reset(mark);
    }

    /// What the bracket is for: a run takes room, the caller gives it back, and the next run
    /// starts where the first one did rather than further along.
    ///
    /// Where it starts and not which address it is handed. Giving room back may hand a whole block
    /// to the allocator, and what comes back next is wherever that allocator answers; what this
    /// promises is that a run repeated a thousand times costs what one costs.
    #[test]
    fn room_taken_since_a_mark_is_taken_again_rather_than_added_to() {
        let mark = souther_mark();
        let _taken = souther_alloc(32);
        let after_one = souther_mark();
        souther_reset(mark);

        for _ in 0..1000 {
            let _taken = souther_alloc(32);
            assert_eq!(souther_mark(), after_one);
            souther_reset(mark);
        }
        assert_eq!(souther_mark(), mark);
    }

    /// A reset takes back what it was asked for and no more.
    #[test]
    fn what_was_taken_before_a_mark_stays_where_it_is() {
        let held = souther_alloc(8).cast::<i64>();
        unsafe { held.write(42) };
        let mark = souther_mark();
        let _dropped = souther_alloc(4096);
        souther_reset(mark);
        assert_eq!(unsafe { held.read() }, 42);
    }

    /// A block's worth at a time, so what is answered crosses the block the arena starts with.
    #[test]
    fn room_past_one_block_is_still_room() {
        let mark = souther_mark();
        let mut held = Vec::new();
        for value in 0..4096i64 {
            let at = souther_alloc(1024).cast::<i64>();
            unsafe { at.write(value) };
            held.push(at);
        }
        for (value, at) in held.iter().enumerate() {
            assert_eq!(unsafe { at.read() }, value as i64);
        }
        souther_reset(mark);
    }

    #[test]
    fn a_string_carries_the_text_it_was_made_from() {
        let mark = souther_mark();
        assert_eq!(said(made("hello")), "hello");
        assert_eq!(said(made("")), "");
        assert_eq!(said(made("\u{0}after a nought")), "\u{0}after a nought");
        souther_reset(mark);
    }

    /// Two strings are equal by what they say. Made separately they stand at two addresses, and an
    /// answer read off the addresses would be the wrong one for exactly this pair.
    #[test]
    fn two_strings_of_one_text_made_separately_are_equal() {
        let mark = souther_mark();
        let one = made("hello");
        let other = made("hello");

        assert_ne!(one, other);
        assert_eq!(souther_string_compare(one, other), 0);
        souther_reset(mark);
    }

    #[test]
    fn a_string_that_begins_another_comes_before_it() {
        let mark = souther_mark();
        assert_eq!(souther_string_compare(made("ab"), made("abc")), -1);
        assert_eq!(souther_string_compare(made("abc"), made("ab")), 1);
        assert_eq!(souther_string_compare(made(""), made("a")), -1);
        souther_reset(mark);
    }

    /// The order is by UTF-16 code unit, which is neither the order of the bytes nor the order of
    /// the code points.
    ///
    /// `𠮷` is U+20BB7 and `￥` is U+FFE5. By code point — which is also what comparing the UTF-8
    /// bytes gives — the first is the greater. As UTF-16 the first begins D842, which is below
    /// FFE5, so the first is the smaller. Both readings are asserted here, so that an
    /// implementation that answered by bytes would fail on the reading it agrees with rather than
    /// on a bare expectation.
    #[test]
    fn text_is_ordered_by_utf_16_code_unit_and_not_by_code_point() {
        let mark = souther_mark();
        let astral = "\u{20bb7}";
        let basic = "\u{ffe5}";

        assert_eq!(souther_string_compare(made(astral), made(basic)), -1);
        assert!(astral.as_bytes() > basic.as_bytes());
        assert!(astral.chars().next() > basic.chars().next());
        souther_reset(mark);
    }

    #[test]
    fn two_strings_joined_say_one_and_then_the_other() {
        let mark = souther_mark();
        assert_eq!(said(souther_string_concat(made("ab"), made("cd"))), "abcd");
        assert_eq!(said(souther_string_concat(made(""), made("cd"))), "cd");
        assert_eq!(said(souther_string_concat(made("ab"), made(""))), "ab");
        souther_reset(mark);
    }

    /// A join makes a third value and leaves both operands as they were. Nothing frees a Souther
    /// value on its own and none of them is modified, so what a caller held before the join it
    /// still holds after it.
    #[test]
    fn a_join_leaves_both_of_its_operands_as_they_were() {
        let mark = souther_mark();
        let before = made("ab");
        let after = made("cd");

        let joined = souther_string_concat(before, after);

        assert_eq!(said(joined), "abcd");
        assert_eq!(said(before), "ab");
        assert_eq!(said(after), "cd");
        souther_reset(mark);
    }

    /// A join is a string like any other, so joining one again is not a different question.
    #[test]
    fn a_joined_string_is_one_that_can_be_joined_and_compared_again() {
        let mark = souther_mark();
        let joined = souther_string_concat(made("ab"), made("cd"));

        assert_eq!(souther_string_compare(joined, made("abcd")), 0);
        assert_eq!(said(souther_string_concat(joined, made("ef"))), "abcdef");
        souther_reset(mark);
    }
}
