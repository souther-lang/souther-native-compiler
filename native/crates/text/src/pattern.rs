//! `String.matches`: whether the whole of a text is one of the strings a pattern denotes (spec
//! §string-patterns).
//!
//! What a pattern denotes is read by the checker and nowhere else, and what crosses is that
//! reading: sets of characters, and strings of them in sequence, in choice and in repetition
//! ([`Part`]). Nothing here reads pattern text, and nothing here decides which readings can be run:
//! every reading the checker makes is one [`compile`] takes, however large its counts.
//!
//! So a count stays a number. The machine is as large as the reading and not as large as what it
//! counts: a repetition counted past one is a loop over its part and a counter, and [`matches`]
//! follows every way through the machine at once, one character at a time, each way holding the
//! counts of the loops it is inside. Two ways at the same step with the same counts are one, so how
//! long a match takes grows with the text and with how many places there are to stand, and never
//! with how many ways there are to be wrong; and whether a match is found does not depend on which
//! way is tried first, which the language says nothing about. A way inside no counted loop, which is
//! every way of most patterns, holds no counts and costs what a step of the machine does.
//!
//! The machine is a run of words, so that the compiler can write it into an object and the runtime
//! read it from there. What a word means is this module's, on both sides.

use crate::Text;
use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;

/// One part of what a pattern denotes, naming the parts it is made of by where they stand in the
/// list it is written in, which is always before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// The one string of no characters.
    Nothing,
    /// No string at all.
    Never,
    /// One character out of these runs, both ends in each, sorted and apart.
    Symbols(Vec<(u32, u32)>),
    /// One after another.
    InTurn(Vec<usize>),
    /// Any one of them.
    EitherOf(Vec<usize>),
    /// The same thing between `least` and `most` times, with no ceiling where `most` is absent.
    Repeated {
        what: usize,
        least: u32,
        most: Option<u32>,
    },
}

/// Parts that are no reading of a pattern: a part names one that does not stand before it, a run
/// is not sorted, apart and of scalar values, a choice is of fewer than two, a ceiling is below its
/// floor, or there are no parts at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotAReading;

/// The machine's steps, each a word saying which and the words after it saying with what.
///
/// `MATCH`: the whole text is read. `SYMBOLS set`: read a character of the set at word `set`.
/// `SPLIT one other`: go on at both. `JUMP to`. `FAIL`: go on nowhere. `ENTER least ceiling most
/// empty body exit`: begin a counted loop of between `least` and, where `ceiling` is one, `most`
/// copies of the part at `body`, which accepts the empty string where `empty` is one, going on at
/// `exit` once done. `NEXT enter`: a copy of the loop entered at `enter` is made.
const MATCH: u32 = 0;
const SYMBOLS: u32 = 1;
const SPLIT: u32 = 2;
const JUMP: u32 = 3;
const FAIL: u32 = 4;
const ENTER: u32 = 5;
const NEXT: u32 = 6;

/// Where the first step is: after how many words there are and where the sets begin.
const FIRST_STEP: usize = 2;

/// Whether the parts are a reading of a pattern.
pub fn check(parts: &[Part]) -> Result<(), NotAReading> {
    if parts.is_empty() {
        return Err(NotAReading);
    }
    for (at, part) in parts.iter().enumerate() {
        let before = |index: &usize| *index < at;
        let fine = match part {
            Part::Nothing | Part::Never => true,
            Part::Symbols(runs) => {
                runs.iter().all(|(from, to)| {
                    from <= to && *to <= 0x10ffff && (*to < 0xd800 || *from > 0xdfff)
                }) && runs.windows(2).all(|pair| pair[0].1 + 1 < pair[1].0)
            }
            Part::InTurn(parts) => parts.iter().all(before),
            Part::EitherOf(arms) => arms.len() >= 2 && arms.iter().all(before),
            Part::Repeated { what, least, most } => {
                before(what) && most.is_none_or(|most| most >= *least)
            }
        };
        if !fine {
            return Err(NotAReading);
        }
    }
    Ok(())
}

/// The machine for the whole pattern, the last of `parts`.
///
/// The words are how many words there are, where the sets begin, the steps, and then each set of
/// characters a step reads: how many runs it is, and each run's two ends. The count comes first so
/// that a reader handed the address of the first word knows how far it may read.
pub fn compile(parts: &[Part]) -> Result<Vec<u32>, NotAReading> {
    compile_within(parts, WRITTEN_OUT)
}

/// How many words a repetition may take written out copy by copy, past which it is a counted loop.
///
/// Which of the two a repetition is written as changes how it is run and not what it accepts: a
/// way through copies written out is told apart by its step alone, which is the cheapest there is
/// to follow, and a count this small costs few words to write out. A larger one would cost words in
/// proportion to what it counts, which is what a counted loop is for.
const WRITTEN_OUT: usize = 256;

fn compile_within(parts: &[Part], written_out: usize) -> Result<Vec<u32>, NotAReading> {
    build(parts, written_out).map(|(machine, _)| machine)
}

/// The machine, and how many times a part was asked to be emitted in writing it.
fn build(parts: &[Part], written_out: usize) -> Result<(Vec<u32>, usize), NotAReading> {
    check(parts)?;
    let mut nullable: Vec<bool> = Vec::with_capacity(parts.len());
    let mut sizes: Vec<usize> = Vec::with_capacity(parts.len());
    for part in parts {
        let empty = match part {
            Part::Nothing => true,
            Part::Never | Part::Symbols(_) => false,
            Part::InTurn(each) => each.iter().all(|at| nullable[*at]),
            Part::EitherOf(arms) => arms.iter().any(|at| nullable[*at]),
            Part::Repeated { what, least, .. } => *least == 0 || nullable[*what],
        };
        nullable.push(empty);
        let size = match part {
            Part::Nothing => 0,
            Part::Never => 1,
            Part::Symbols(_) => 2,
            Part::InTurn(each) => each
                .iter()
                .map(|at| sizes[*at])
                .fold(0, usize::saturating_add),
            Part::EitherOf(arms) => {
                arms.iter()
                    .map(|at| sizes[*at].saturating_add(5))
                    .fold(0, usize::saturating_add)
                    - 5
            }
            Part::Repeated { what, least, most } => {
                Form::of(*least, *most, sizes[*what], written_out).size(sizes[*what])
            }
        };
        sizes.push(size);
    }
    let mut building = Building {
        parts,
        nullable: &nullable,
        sizes: &sizes,
        written_out,
        words: vec![0, 0],
        sets: Vec::new(),
        set_of: vec![None; parts.len()],
        emitted: 0,
    };
    building.emit(parts.len() - 1);
    building.words.push(MATCH);
    // Writing the machine is work in proportion to what it writes, and never to what a part
    // counts: every part emitted writes a word or stands in the parts of one that does, and one
    // asked for that writes none is asked for by one that does. A repetition is written out only
    // where its part writes words, so its copies are no more than the words they write.
    debug_assert!(
        building.emitted <= building.words.len() * (parts.len() + 1) * (parts.len() + 1),
        "{} parts emitted for {} words of {} parts",
        building.emitted,
        building.words.len(),
        parts.len()
    );
    let emitted = building.emitted;
    Ok((building.finished(), emitted))
}

/// How a repetition is written.
#[derive(Clone, Copy)]
enum Form {
    /// Nothing: the part writes no words, so it accepts the empty string alone, and so does any
    /// number of copies of it. Its counts are never read, so none of them is work.
    Empty,
    /// Its part, as many times as the floor, and after that another copy or none as often as the
    /// ceiling allows, or a loop of it where there is none.
    WrittenOut { least: u32, most: Option<u32> },
    /// A loop of its part counting its copies.
    Counted,
}

impl Form {
    fn of(least: u32, most: Option<u32>, part: usize, written_out: usize) -> Form {
        if part == 0 {
            return Form::Empty;
        }
        let out = Form::WrittenOut { least, most };
        if out.size(part) <= written_out {
            out
        } else {
            Form::Counted
        }
    }

    /// How many words it takes, where its part takes `part`.
    fn size(self, part: usize) -> usize {
        match self {
            Form::Empty => 0,
            Form::WrittenOut { least, most } => {
                let floor = (least as usize).saturating_mul(part);
                let rest = match most {
                    // A split, the part and a jump back.
                    None => part.saturating_add(5),
                    // A split before each copy past the floor.
                    Some(most) => ((most - least) as usize).saturating_mul(part.saturating_add(3)),
                };
                floor.saturating_add(rest)
            }
            // Entering the loop, the part, and the copy made.
            Form::Counted => part.saturating_add(9),
        }
    }
}

/// A set of characters a step reads, and the words of the steps reading it, which are pointed at
/// it once it is written.
type SetRead<'a> = (&'a [(u32, u32)], Vec<usize>);

struct Building<'a> {
    parts: &'a [Part],
    nullable: &'a [bool],
    sizes: &'a [usize],
    written_out: usize,
    words: Vec<u32>,
    /// Each set a step reads, and the steps reading it, to be pointed at it once it is written.
    sets: Vec<SetRead<'a>>,
    /// Which of `sets` a part's set is, once it has been met: a part inside a loop is written once
    /// however many times it is read.
    set_of: Vec<Option<usize>>,
    /// How many times a part has been asked to be emitted, those writing nothing among them.
    emitted: usize,
}

impl<'a> Building<'a> {
    fn here(&self) -> u32 {
        self.words.len() as u32
    }

    fn emit(&mut self, at: usize) {
        self.emitted += 1;
        // A part writing no words accepts the empty string and nothing else, which is what writing
        // nothing does, so nothing is emitted for it.
        if self.sizes[at] == 0 {
            return;
        }
        let parts = self.parts;
        match &parts[at] {
            Part::Nothing => {}
            Part::Never => self.words.push(FAIL),
            Part::Symbols(runs) => {
                let set = *self.set_of[at].get_or_insert_with(|| {
                    self.sets.push((runs, Vec::new()));
                    self.sets.len() - 1
                });
                self.words.push(SYMBOLS);
                self.sets[set].1.push(self.words.len());
                self.words.push(0);
            }
            Part::InTurn(each) => {
                for part in each {
                    self.emit(*part);
                }
            }
            Part::EitherOf(arms) => {
                let mut jumps = Vec::new();
                for (index, arm) in arms.iter().enumerate() {
                    if index + 1 < arms.len() {
                        let split = self.words.len();
                        self.words.extend([SPLIT, 0, 0]);
                        self.words[split + 1] = self.here();
                        self.emit(*arm);
                        jumps.push(self.words.len() + 1);
                        self.words.extend([JUMP, 0]);
                        self.words[split + 2] = self.here();
                    } else {
                        self.emit(*arm);
                    }
                }
                let end = self.here();
                for jump in jumps {
                    self.words[jump] = end;
                }
            }
            Part::Repeated { what, least, most } => {
                let form = Form::of(*least, *most, self.sizes[*what], self.written_out);
                match form {
                    Form::Empty => unreachable!("a repetition writing no words is not emitted"),
                    Form::WrittenOut { least, most } => {
                        for _ in 0..least {
                            self.emit(*what);
                        }
                        match most {
                            // A way comes back to the same step, and two ways at one step are one,
                            // so a loop with no ceiling needs no count.
                            None => {
                                let split = self.words.len();
                                self.words.extend([SPLIT, 0, 0]);
                                self.words[split + 1] = self.here();
                                self.emit(*what);
                                self.words.extend([JUMP, split as u32]);
                                self.words[split + 2] = self.here();
                            }
                            Some(most) => {
                                let mut splits = Vec::new();
                                for _ in least..most {
                                    let split = self.words.len();
                                    self.words.extend([SPLIT, 0, 0]);
                                    self.words[split + 1] = self.here();
                                    splits.push(split);
                                    self.emit(*what);
                                }
                                let end = self.here();
                                for split in splits {
                                    self.words[split + 2] = end;
                                }
                            }
                        }
                    }
                    Form::Counted => {
                        let enter = self.words.len();
                        self.words.extend([
                            ENTER,
                            *least,
                            u32::from(most.is_some()),
                            most.unwrap_or(0),
                            u32::from(self.nullable[*what]),
                            0,
                            0,
                        ]);
                        self.words[enter + 5] = self.here();
                        self.emit(*what);
                        self.words.extend([NEXT, enter as u32]);
                        self.words[enter + 6] = self.here();
                    }
                }
            }
        }
    }

    /// The words, with the sets written after the steps and each step reading one pointed at it.
    fn finished(mut self) -> Vec<u32> {
        self.words[1] = self.here();
        for (runs, readers) in core::mem::take(&mut self.sets) {
            let at = self.here();
            for reader in readers {
                self.words[reader] = at;
            }
            self.words.push(runs.len() as u32);
            for (from, to) in runs {
                self.words.push(*from);
                self.words.push(*to);
            }
        }
        self.words[0] = self.words.len() as u32;
        self.words
    }
}

/// How many words a machine is, read off its first word.
pub fn length(first: u32) -> usize {
    first as usize
}

/// Whether the set at word `set` holds the character.
fn holds(machine: &[u32], set: usize, point: u32) -> bool {
    let runs = machine[set] as usize;
    let (mut low, mut high) = (0, runs);
    while low < high {
        let middle = (low + high) / 2;
        let (from, to) = (machine[set + 1 + 2 * middle], machine[set + 2 + 2 * middle]);
        if point < from {
            high = middle;
        } else if point > to {
            low = middle + 1;
        } else {
            return true;
        }
    }
    false
}

/// How many copies a way has made of each counted loop it is inside, the innermost last, and
/// whether it began a copy of that loop since the last character was read: a way coming back to
/// the loop with that still so made a copy of nothing, which is not a copy, and goes no further.
type Counts = Vec<(u32, bool)>;

/// One way through the machine: the step it is at and its counts.
struct Way {
    at: usize,
    counts: Counts,
}

/// The ways waiting at a step that reads a character, each once.
///
/// A way holding no counts is told apart by its step alone, which is the case for every way of a
/// pattern with no count past one; the rest by their step and counts.
struct Ways {
    plain: Vec<usize>,
    counted: Vec<(usize, Counts)>,
    /// The character at which each step last had a plain way added, so that it is added once.
    added: Vec<usize>,
    /// Every step and counts a way has been at since the last character.
    seen: BTreeSet<(usize, Counts)>,
    generation: usize,
    accepting: bool,
    /// Ways still to be followed, kept from one character to the next for their room: those
    /// holding no counts as the step they are at.
    plain_pending: Vec<usize>,
    pending: Vec<Way>,
}

impl Ways {
    fn new(steps: usize) -> Ways {
        Ways {
            plain: Vec::new(),
            counted: Vec::new(),
            added: vec![usize::MAX; steps],
            seen: BTreeSet::new(),
            generation: 0,
            accepting: false,
            plain_pending: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// The next character's ways, empty, keeping what telling plain ways apart takes.
    fn clear(&mut self, generation: usize) {
        self.plain.clear();
        self.counted.clear();
        self.seen.clear();
        self.generation = generation;
        self.accepting = false;
    }
}

/// Every way `from` goes without reading a character, added to `into`.
///
/// A way holding no counts is followed as the step it is at and nothing more, which is every way
/// of a pattern with no counted loop; it holds counts only inside one.
fn follow(machine: &[u32], from: Way, into: &mut Ways) {
    if from.counts.is_empty() {
        into.plain_pending.push(from.at);
    } else {
        into.pending.push(from);
    }
    follow_pending(machine, into);
}

/// Every way waiting in `into` to be followed, followed.
fn follow_pending(machine: &[u32], into: &mut Ways) {
    let mut plain = core::mem::take(&mut into.plain_pending);
    let mut counted = core::mem::take(&mut into.pending);
    loop {
        if let Some(at) = plain.pop() {
            if into.added[at] == into.generation {
                continue;
            }
            into.added[at] = into.generation;
            match machine[at] {
                MATCH => into.accepting = true,
                SYMBOLS => into.plain.push(at),
                SPLIT => {
                    plain.push(machine[at + 2] as usize);
                    plain.push(machine[at + 1] as usize);
                }
                JUMP => plain.push(machine[at + 1] as usize),
                FAIL => {}
                ENTER => {
                    let way = Way {
                        at,
                        counts: vec![(0, false)],
                    };
                    decide(machine, at, way, &mut plain, &mut counted);
                }
                step => unreachable!("a step {step} a way holding no counts is never at"),
            }
            continue;
        }
        let Some(mut way) = counted.pop() else {
            break;
        };
        if !into.seen.insert((way.at, way.counts.clone())) {
            continue;
        }
        let at = way.at;
        match machine[at] {
            MATCH => into.accepting = true,
            SYMBOLS => into.counted.push((way.at, way.counts)),
            SPLIT => {
                counted.push(Way {
                    at: machine[at + 2] as usize,
                    counts: way.counts.clone(),
                });
                way.at = machine[at + 1] as usize;
                counted.push(way);
            }
            JUMP => {
                way.at = machine[at + 1] as usize;
                counted.push(way);
            }
            FAIL => {}
            ENTER => {
                way.counts.push((0, false));
                decide(machine, at, way, &mut plain, &mut counted);
            }
            NEXT => {
                let enter = machine[at + 1] as usize;
                let (made, begun) = way.counts.pop().expect("a copy is made inside its loop");
                // A copy of nothing is not a copy.
                if begun {
                    continue;
                }
                // Past the floor, how many more there were says nothing where there is no
                // ceiling, so they are not told apart.
                let made = match machine[enter + 2] {
                    0 => (made + 1).min(machine[enter + 1]),
                    _ => made + 1,
                };
                way.counts.push((made, false));
                decide(machine, enter, way, &mut plain, &mut counted);
            }
            step => unreachable!("a step {step} is none `compile` writes"),
        }
    }
    into.plain_pending = plain;
    into.pending = counted;
}

/// Where a way at the loop entered at `enter` goes: out of it where enough copies are made, and
/// into another copy where the ceiling allows one. A way leaving its last loop holds no counts.
fn decide(
    machine: &[u32],
    enter: usize,
    mut way: Way,
    plain: &mut Vec<usize>,
    counted: &mut Vec<Way>,
) {
    let least = machine[enter + 1];
    let most = (machine[enter + 2] != 0).then_some(machine[enter + 3]);
    let empty = machine[enter + 4] != 0;
    let (made, _) = *way.counts.last().expect("a loop's way counts its copies");
    // The copies still owed can be copies of nothing, where the part accepts nothing.
    if made >= least || empty {
        let exit = machine[enter + 6] as usize;
        if way.counts.len() == 1 {
            plain.push(exit);
        } else {
            let mut counts = way.counts.clone();
            counts.pop();
            counted.push(Way { at: exit, counts });
        }
    }
    if most.is_none_or(|most| made < most) {
        *way.counts.last_mut().expect("counted above") = (made, true);
        way.at = machine[enter + 5] as usize;
        counted.push(way);
    }
}

/// Whether the whole of the text is a string the machine accepts.
///
/// # Panics
///
/// Where `machine` is not words [`compile`] answered.
pub fn matches(machine: &[u32], text: Text) -> bool {
    let steps = machine[1] as usize;
    let mut now = Ways::new(steps);
    // One record of which steps have had a plain way added, told apart by character, handed on
    // from one character's ways to the next.
    let mut next = Ways::new(0);
    let start = Way {
        at: FIRST_STEP,
        counts: Vec::new(),
    };
    follow(machine, start, &mut now);
    for (read, point) in text.as_str().chars().map(u32::from).enumerate() {
        next.clear(read + 1);
        next.added = core::mem::take(&mut now.added);
        for &at in &now.plain {
            if holds(machine, machine[at + 1] as usize, point) {
                next.plain_pending.push(at + 2);
            }
        }
        follow_pending(machine, &mut next);
        for (at, counts) in &now.counted {
            if holds(machine, machine[at + 1] as usize, point) {
                // A character is read, so every copy begun before it has made something.
                let counts = counts.iter().map(|(made, _)| (*made, false)).collect();
                follow(machine, Way { at: at + 2, counts }, &mut next);
            }
        }
        if next.plain.is_empty() && next.counted.is_empty() && !next.accepting {
            return false;
        }
        core::mem::swap(&mut now, &mut next);
    }
    now.accepting
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::string::String;

    fn one(point: char) -> Part {
        Part::Symbols(vec![(point as u32, point as u32)])
    }

    fn digit() -> Part {
        Part::Symbols(vec![(0x30, 0x39)])
    }

    fn counted(what: usize, least: u32, most: Option<u32>) -> Part {
        Part::Repeated { what, least, most }
    }

    fn accepts(parts: &[Part], text: &str) -> bool {
        matches(&compile(parts).expect("a machine"), Text::held(text))
    }

    /// `[0-9]{3}-[0-9]{4}`.
    #[test]
    fn a_postal_code_is_matched_whole() {
        let parts = [
            digit(),
            counted(0, 3, Some(3)),
            one('-'),
            counted(0, 4, Some(4)),
            Part::InTurn(vec![1, 2, 3]),
        ];
        assert!(accepts(&parts, "123-4567"));
        assert!(!accepts(&parts, "123-456"));
        assert!(!accepts(&parts, "123-45678"));
        assert!(!accepts(&parts, "x123-4567"));
        assert!(!accepts(&parts, ""));
    }

    /// `(a|bc)*d?`, over a character past the basic plane too.
    #[test]
    fn a_choice_repeated_and_an_optional_one() {
        let parts = [
            one('a'),
            one('b'),
            one('c'),
            Part::InTurn(vec![1, 2]),
            Part::EitherOf(vec![0, 3]),
            counted(4, 0, None),
            one('𠮷'),
            counted(6, 0, Some(1)),
            Part::InTurn(vec![5, 7]),
        ];
        for text in ["", "a", "bc", "abca", "abc𠮷", "𠮷"] {
            assert!(accepts(&parts, text), "{text}");
        }
        for text in ["b", "c", "ab𠮷", "𠮷𠮷", "d"] {
            assert!(!accepts(&parts, text), "{text}");
        }
    }

    /// A repetition of something that may be empty does not go round for ever: `(a*)*`.
    #[test]
    fn a_repetition_of_what_may_be_empty_ends() {
        let parts = [one('a'), counted(0, 0, None), counted(1, 0, None)];
        assert!(accepts(&parts, ""));
        assert!(accepts(&parts, "aaaa"));
        assert!(!accepts(&parts, "ab"));
    }

    /// Copies of nothing fill a floor, and do not count against a ceiling: `(a?){3}` takes from
    /// none to three `a`s, and `(a?){2,1000000}` takes two quickly however high its ceiling.
    #[test]
    fn copies_of_nothing_fill_a_floor_and_not_a_ceiling() {
        let parts = [one('a'), counted(0, 0, Some(1)), counted(1, 3, Some(3))];
        for (text, taken) in [("", true), ("a", true), ("aaa", true), ("aaaa", false)] {
            assert_eq!(accepts(&parts, text), taken, "{text}");
        }
        let parts = [
            one('a'),
            counted(0, 0, Some(1)),
            counted(1, 2, Some(1_000_000)),
        ];
        assert!(accepts(&parts, "aa"));
        assert!(accepts(&parts, ""));
    }

    /// A count is a number the machine holds and not copies it is made of, so no count the language
    /// reads is too large: `a{1048576}` is a machine as long as `a{1000}`, and counts inside counts
    /// are no longer than the loops they are written as. What a count accepts is exactly that many.
    #[test]
    fn a_count_is_held_and_not_copied() {
        let machine = compile(&[one('a'), counted(0, 1 << 20, Some(1 << 20))]).expect("a machine");
        let few = compile(&[one('a'), counted(0, 1000, Some(1000))]).expect("a machine");
        assert_eq!(machine.len(), few.len());
        assert!(!matches(&machine, Text::held("aaa")));
        let nested = [
            one('a'),
            counted(0, 1000, Some(1000)),
            counted(1, 1000, Some(1000)),
            counted(2, 1000, Some(1000)),
        ];
        assert!(compile(&nested).expect("a machine").len() < 3 * few.len());
        assert!(!accepts(&nested, "aaa"));
        let parts = [one('a'), counted(0, 300, Some(300))];
        assert!(accepts(&parts, &"a".repeat(300)));
        assert!(!accepts(&parts, &"a".repeat(299)));
        assert!(!accepts(&parts, &"a".repeat(301)));
    }

    /// Where a part read from `start` can end, by what the parts mean and nothing else: the
    /// reference the machines are held to.
    fn ends(parts: &[Part], part: usize, text: &[u32], start: usize) -> BTreeSet<usize> {
        let after = |from: &BTreeSet<usize>, part: usize| -> BTreeSet<usize> {
            from.iter()
                .flat_map(|at| ends(parts, part, text, *at))
                .collect()
        };
        match &parts[part] {
            Part::Nothing => BTreeSet::from([start]),
            Part::Never => BTreeSet::new(),
            Part::Symbols(runs) => match text.get(start) {
                Some(point) if runs.iter().any(|(from, to)| from <= point && point <= to) => {
                    BTreeSet::from([start + 1])
                }
                _ => BTreeSet::new(),
            },
            Part::InTurn(each) => each.iter().fold(BTreeSet::from([start]), |reached, part| {
                after(&reached, *part)
            }),
            Part::EitherOf(arms) => arms
                .iter()
                .flat_map(|arm| ends(parts, *arm, text, start))
                .collect(),
            // Every copy past the floor adds where it can end. With no ceiling, a copy that read
            // nothing reaches nowhere new, so as many as the text is long, and one more, reach all.
            Part::Repeated { what, least, most } => {
                let last = most.unwrap_or(*least + text.len() as u32 + 1);
                let mut reached = BTreeSet::from([start]);
                let mut all = BTreeSet::new();
                for copies in 0..=last {
                    if copies >= *least {
                        all.extend(reached.iter().copied());
                    }
                    if copies == last || reached.is_empty() {
                        break;
                    }
                    reached = after(&reached, *what);
                }
                all
            }
        }
    }

    /// A counted loop and the copies it would be written out as accept the same strings, and both
    /// accept what the parts mean: over random patterns of two letters, with counts, choices,
    /// optional and empty parts, and random texts.
    #[test]
    fn counted_and_written_out_accept_what_the_parts_mean() {
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = |bound: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % bound as u64) as usize
        };
        for _ in 0..400 {
            let mut parts = vec![one('a'), one('b'), Part::Symbols(vec![(97, 98)])];
            for _ in 0..next(6) {
                let known = parts.len();
                let part = match next(5) {
                    0 => Part::Nothing,
                    1 => Part::InTurn((0..1 + next(3)).map(|_| next(known)).collect()),
                    2 => Part::EitherOf(vec![next(known), next(known)]),
                    _ => {
                        let least = next(4) as u32;
                        let most = match next(3) {
                            0 => None,
                            _ => Some(least + next(4) as u32),
                        };
                        counted(next(known), least, most)
                    }
                };
                parts.push(part);
            }
            let counted_all = compile_within(&parts, 0).expect("a machine");
            let written_all = compile_within(&parts, usize::MAX).expect("a machine");
            for _ in 0..20 {
                let text: String = (0..next(7)).map(|_| ['a', 'b'][next(2)]).collect();
                let points: Vec<u32> = text.chars().map(u32::from).collect();
                let meant = ends(&parts, parts.len() - 1, &points, 0).contains(&points.len());
                assert_eq!(
                    matches(&counted_all, Text::held(&text)),
                    meant,
                    "{parts:?} {text}"
                );
                assert_eq!(
                    matches(&written_all, Text::held(&text)),
                    meant,
                    "{parts:?} {text}"
                );
            }
        }
    }

    /// A repetition of what writes no words is nothing: written as no words, and at once, however
    /// much it counts, with or without a ceiling, and accepting the empty string alone. Writing it
    /// is the same work whether it counts one or a hundred million.
    #[test]
    fn a_repetition_of_nothing_is_nothing_whatever_it_counts() {
        let shapes = |count: u32| {
            [
                vec![Part::Nothing, counted(0, count, Some(count))],
                vec![Part::Nothing, counted(0, 0, Some(count))],
                vec![Part::Nothing, counted(0, count, None)],
                vec![
                    Part::Nothing,
                    counted(0, count, Some(count)),
                    counted(1, count, None),
                ],
                vec![
                    Part::Nothing,
                    Part::InTurn(vec![0, 0, 0]),
                    counted(1, count, None),
                ],
                vec![one('a'), counted(0, 0, Some(0)), counted(1, count, None)],
            ]
        };
        let only_match = compile(&[Part::Nothing]).expect("a machine");
        for (one_copy, many) in shapes(1).into_iter().zip(shapes(100_000_000)) {
            let (machine, work) = build(&many, WRITTEN_OUT).expect("a machine");
            let (_, work_of_one) = build(&one_copy, WRITTEN_OUT).expect("a machine");
            assert_eq!(machine, only_match, "{many:?}");
            assert_eq!(work, work_of_one, "{many:?}");
            assert!(matches(&machine, Text::held("")), "{many:?}");
            assert!(!matches(&machine, Text::held("a")), "{many:?}");
        }
    }

    /// Nothing accepts the empty string alone, and never accepts nothing.
    #[test]
    fn nothing_and_never() {
        assert!(accepts(&[Part::Nothing], ""));
        assert!(!accepts(&[Part::Nothing], "a"));
        assert!(!accepts(&[Part::Never], ""));
        let parts = [Part::Never, one('a'), Part::EitherOf(vec![0, 1])];
        assert!(accepts(&parts, "a"));
    }

    /// A reading that names a part after itself, or a run through the surrogates, is refused.
    #[test]
    fn what_is_no_reading_is_refused() {
        assert_eq!(compile(&[]), Err(NotAReading));
        assert_eq!(compile(&[Part::InTurn(vec![0])]), Err(NotAReading));
        assert_eq!(
            compile(&[Part::Symbols(vec![(0xd000, 0xe000)])]),
            Err(NotAReading)
        );
        assert_eq!(
            compile(&[Part::Symbols(vec![(5, 9), (10, 12)])]),
            Err(NotAReading)
        );
        assert_eq!(
            compile(&[one('a'), counted(0, 3, Some(2))]),
            Err(NotAReading)
        );
    }
}
