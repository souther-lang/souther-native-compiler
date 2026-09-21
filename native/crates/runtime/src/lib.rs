//! What a Souther program calls that is not its own code.
//!
//! Room, so far. A value made of fields has to live somewhere, and generated code takes that room
//! from here rather than deciding for itself where a value goes.
//!
//! Where a computation here turns out to be the one the wasm runtime already does — a calendar, a
//! regular expression — it is lifted into something both read. That is done when the second copy
//! exists and not before: until then there is nothing to tell a shared meaning from a shared
//! spelling.

use souther_native_abi::SLOT;
use std::cell::RefCell;

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

#[cfg(test)]
mod tests {
    use super::{souther_alloc, souther_mark, souther_reset};

    use souther_native_abi::SLOT;

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
}
