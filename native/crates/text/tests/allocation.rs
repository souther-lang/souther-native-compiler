//! Text no carrier holds is never made.
//!
//! An operation that ends because what it would build is more than the carrier holds must end
//! before it has built it, and not after: a check on a finished result is no check on memory, and
//! a run that is to end with `RequiredFormHasNoPlace` would end for want of memory first. The
//! outcome cannot tell the two apart, so this counts what is allocated while an operation runs, and
//! holds it to a small fraction of the capacity it was refused for.
//!
//! A counting allocator sees every thread, so this is one test that runs its cases one after
//! another, and the binary holds no other.

use souther_text::{
    Capacity, Text, append, join, lowercase, pad_left, pad_right, repeat, replace, reverse,
    uppercase,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = NOW.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
        PEAK.fetch_max(now, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, at: *mut u8, layout: Layout) {
        NOW.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(at, layout) }
    }

    unsafe fn realloc(&self, at: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if size > layout.size() {
            let now = NOW.fetch_add(size - layout.size(), Ordering::SeqCst) + size - layout.size();
            PEAK.fetch_max(now, Ordering::SeqCst);
        } else {
            NOW.fetch_sub(layout.size() - size, Ordering::SeqCst);
        }
        unsafe { System.realloc(at, layout, size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// What is held: small, so that what is refused is large beside it, and what is allocated on the
/// way to refusing it is told from what was built.
const HELD: i64 = 1 << 16;

/// The most an operation may allocate on the way to refusing what it was asked for: a few times
/// what is held, and a small fraction of what it was asked for.
const AT_MOST: usize = 4 * HELD as usize;

/// The most an operation the language defines as measured before it is built may allocate: none of
/// its text, whatever is held. Joins, repeats and pads state what they come to before writing the
/// first of it, and a budget spent as text is written would have written what is held by then.
const BEFORE_ANY: usize = 4096;

/// What the operation allocated at its peak, beyond what was already there, and what it answered.
fn spending<T>(operation: impl FnOnce() -> T) -> (usize, T) {
    let before = NOW.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let answered = operation();
    (PEAK.load(Ordering::SeqCst) - before, answered)
}

#[test]
fn what_no_carrier_holds_is_never_built() {
    let capacity = Capacity::of_code_points(HELD);
    // Made before anything is measured: the operands are text already held, and are not what is
    // asked about.
    let big = "a".repeat(1 << 22);
    let astral = "\u{10000}".repeat(1 << 20);
    let sharp = "ß".repeat(1 << 21);
    let many = "a,".repeat(1 << 20);
    // `Text::held` asks a debug build whether its text is in NFC, which is the whole of it once, so
    // every operand is made here and none inside what is measured.
    let (big, astral, sharp, many) = (
        Text::held(&big),
        Text::held(&astral),
        Text::held(&sharp),
        Text::held(&many),
    );
    let (a, b, comma, empty, bb) = (
        Text::held("a"),
        Text::held("b"),
        Text::held(","),
        Text::held(""),
        Text::held("bb"),
    );
    let short = Text::held(&big.as_str()[..1 << 12]);
    let long = Text::held(&big.as_str()[..1 << 10]);
    let plane = Text::held("\u{10000}");
    let pieces: Vec<Text> = (0..64).map(|_| short).collect();

    type Case<'a> = (&'a str, usize, Box<dyn Fn() -> Option<String> + 'a>);
    let cases: Vec<Case> = vec![
        (
            "append",
            BEFORE_ANY,
            Box::new(|| append(big, big, capacity)),
        ),
        (
            "join",
            AT_MOST,
            Box::new(|| join(comma, pieces.iter().copied(), capacity)),
        ),
        (
            "replace",
            AT_MOST,
            Box::new(|| replace(a, bb, big, capacity)),
        ),
        (
            "replace, many pieces",
            AT_MOST,
            Box::new(|| replace(comma, empty, many, capacity)),
        ),
        ("reverse", AT_MOST, Box::new(|| reverse(astral, capacity))),
        (
            "lowercase",
            AT_MOST,
            Box::new(|| lowercase(sharp, capacity)),
        ),
        (
            "uppercase",
            AT_MOST,
            Box::new(|| uppercase(sharp, capacity)),
        ),
        (
            "repeat, many copies",
            BEFORE_ANY,
            Box::new(|| repeat(1 << 40, a, capacity)),
        ),
        (
            "repeat, a long text",
            BEFORE_ANY,
            Box::new(|| repeat(1 << 20, long, capacity)),
        ),
        (
            "padLeft",
            BEFORE_ANY,
            Box::new(|| pad_left(1 << 40, a, b, capacity)),
        ),
        (
            "padRight, a fill of astral characters",
            BEFORE_ANY,
            // The width alone is past what `capacity` holds — every code point of the fill is
            // still one code point, however many bytes or UTF-16 units it takes.
            Box::new(|| pad_right(HELD + 1, plane, b, capacity)),
        ),
    ];
    for (name, at_most, operation) in cases {
        let (peak, answered) = spending(operation);
        assert_eq!(answered, None, "{name} is more than is held");
        assert!(
            peak <= at_most,
            "{name} allocated {peak} bytes on the way to refusing what was more than {HELD} units"
        );
    }
}
