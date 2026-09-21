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
const BLOCK: usize = 1 << 20;

thread_local! {
    static ARENA: RefCell<Arena> = const { RefCell::new(Arena::new()) };
}

struct Arena {
    blocks: Vec<Vec<u8>>,
    taken: usize,
}

impl Arena {
    const fn new() -> Self {
        Arena {
            blocks: Vec::new(),
            taken: 0,
        }
    }

    /// Room for `size` bytes, aligned to a slot.
    ///
    /// Blocks are never moved and never given back while a mark below them stands, so a pointer
    /// this answered stays where it is until that mark is reset. A vector of blocks rather than one
    /// growing block for that reason: growing one would move what a caller is holding.
    fn room(&mut self, size: usize) -> *mut u8 {
        let wanted = size.next_multiple_of(SLOT as usize).max(SLOT as usize);
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
        unsafe { block.as_mut_ptr().add(at) }
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
#[unsafe(no_mangle)]
pub extern "C" fn souther_alloc(size: i64) -> *mut u8 {
    ARENA.with(|it| it.borrow_mut().room(size.max(0) as usize))
}

/// Where the arena stands, for a caller about to bracket a call.
#[unsafe(no_mangle)]
pub extern "C" fn souther_mark() -> i64 {
    ARENA.with(|it| it.borrow().mark() as i64)
}

/// Drops what a call made, back to `mark`.
#[unsafe(no_mangle)]
pub extern "C" fn souther_reset(mark: i64) {
    ARENA.with(|it| it.borrow_mut().reset(mark.max(0) as usize));
}

#[cfg(test)]
mod tests {
    use super::{souther_alloc, souther_mark, souther_reset};

    #[test]
    fn room_answered_twice_is_two_different_places() {
        let one = souther_alloc(8);
        let other = souther_alloc(8);
        assert_ne!(one, other);
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

    /// What the bracket is for: a run takes room, the caller gives it back, and the next run is
    /// where the first one started rather than further along.
    #[test]
    fn room_taken_since_a_mark_is_there_to_take_again() {
        let mark = souther_mark();
        let first = souther_alloc(32);
        souther_reset(mark);
        let again = souther_alloc(32);
        assert_eq!(first, again);
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
