//! `String.matches`: whether the whole of a text is one of the strings a pattern denotes (spec
//! §string-patterns).
//!
//! What a pattern denotes is read by the checker and nowhere else, and what crosses is that
//! reading: sets of characters, and strings of them in sequence, in choice and in repetition
//! ([`Part`]). Nothing here reads pattern text. What is built of the reading is a machine
//! ([`compile`]) and what runs it is [`matches`], which follows every way through the machine at
//! once, one character at a time: how long it takes grows with the text and the machine and never
//! with how many ways there are to be wrong, and whether a match is found does not depend on which
//! way is tried first, which the language says nothing about.
//!
//! The machine is a run of words, so that the compiler can write it into an object and the runtime
//! read it from there. What a word means is this module's, on both sides.

use crate::scalar_values;
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

/// Why a pattern's parts built no machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The parts are not a reading of any pattern: a part names one that does not stand before
    /// it, a run is not sorted, apart and of scalar values, a choice is of fewer than two, or a
    /// ceiling is below its floor.
    NotAReading,
    /// The machine would have more steps than [`MOST_STEPS`]: a count repeats what it counts, so
    /// counts inside counts multiply.
    TooLarge,
}

/// How many steps a machine may have.
pub const MOST_STEPS: usize = 1 << 20;

const MATCH: u32 = 0;
const SYMBOLS: u32 = 1;
const SPLIT: u32 = 2;
const JUMP: u32 = 3;
const FAIL: u32 = 4;

/// A step is three words: what it does, and two it does it with.
const STEP: usize = 3;

/// Where the first step is: after how many words there are and how many steps.
const FIRST_STEP: usize = 2;

/// A machine for the whole pattern, the last of `parts`.
///
/// The words are how many words there are, the number of steps, the steps, and after them each set
/// of characters a step reads: how many runs it is, and each run's two ends. A step reading a set
/// holds where the set starts. The count comes first so that a reader handed the address of the
/// first word knows how far it may read.
pub fn compile(parts: &[Part]) -> Result<Vec<u32>, Refused> {
    let steps = check(parts)?;
    let whole = parts.len() - 1;
    let mut building = Building {
        parts,
        steps: Vec::with_capacity(steps),
        sets: Vec::new(),
        set_of: vec![None; parts.len()],
    };
    building.emit(whole);
    building.push(MATCH, 0, 0);
    Ok(building.words())
}

/// Whether the parts are a reading of a pattern whose machine is within [`MOST_STEPS`], and how
/// many steps it is: what [`compile`] would refuse, found without building anything.
pub fn check(parts: &[Part]) -> Result<usize, Refused> {
    if parts.is_empty() {
        return Err(Refused::NotAReading);
    }
    for (at, part) in parts.iter().enumerate() {
        well_formed(at, part)?;
    }
    let mut sizes = Vec::with_capacity(parts.len());
    for part in parts {
        let size = steps(part, &sizes);
        sizes.push(size);
    }
    let whole = sizes[parts.len() - 1] + 1;
    if whole > MOST_STEPS {
        return Err(Refused::TooLarge);
    }
    Ok(whole)
}

fn well_formed(at: usize, part: &Part) -> Result<(), Refused> {
    let before = |index: &usize| *index < at;
    let fine = match part {
        Part::Nothing | Part::Never => true,
        Part::Symbols(runs) => {
            runs.iter()
                .all(|(from, to)| from <= to && *to <= 0x10ffff && (*to < 0xd800 || *from > 0xdfff))
                && runs.windows(2).all(|pair| pair[0].1 + 1 < pair[1].0)
        }
        Part::InTurn(parts) => parts.iter().all(before),
        Part::EitherOf(arms) => arms.len() >= 2 && arms.iter().all(before),
        Part::Repeated { what, least, most } => {
            before(what) && most.is_none_or(|most| most >= *least)
        }
    };
    if fine {
        Ok(())
    } else {
        Err(Refused::NotAReading)
    }
}

/// How many steps a part is emitted as, given how many each part before it is, never past
/// [`MOST_STEPS`] however large the product would be.
fn steps(part: &Part, sizes: &[usize]) -> usize {
    let bounded = |it: usize| it.min(MOST_STEPS);
    bounded(match part {
        Part::Nothing => 0,
        Part::Never | Part::Symbols(_) => 1,
        Part::InTurn(parts) => parts.iter().map(|at| sizes[*at]).sum(),
        // A split before every arm but the last, and a jump after every arm but the last.
        Part::EitherOf(arms) => arms.iter().map(|at| sizes[*at] + 2).sum::<usize>() - 2,
        Part::Repeated { what, least, most } => {
            let one = sizes[*what];
            let floor = (*least as usize).saturating_mul(one);
            let rest = match most {
                // A split, the thing, and a jump back.
                None => one + 2,
                // A split before each copy past the floor.
                Some(most) => ((most - least) as usize).saturating_mul(one + 1),
            };
            floor.saturating_add(rest)
        }
    })
}

struct Building<'a> {
    parts: &'a [Part],
    steps: Vec<[u32; STEP]>,
    sets: Vec<&'a [(u32, u32)]>,
    /// Which of `sets` a part reading one is, once it has been written: a count repeats a part and
    /// not its set.
    set_of: Vec<Option<u32>>,
}

impl<'a> Building<'a> {
    fn push(&mut self, what: u32, one: u32, other: u32) -> usize {
        self.steps.push([what, one, other]);
        self.steps.len() - 1
    }

    fn here(&self) -> u32 {
        self.steps.len() as u32
    }

    fn emit(&mut self, at: usize) {
        let parts = self.parts;
        match &parts[at] {
            Part::Nothing => {}
            Part::Never => {
                self.push(FAIL, 0, 0);
            }
            Part::Symbols(runs) => {
                let set = match self.set_of[at] {
                    Some(set) => set,
                    None => {
                        self.sets.push(runs);
                        let set = (self.sets.len() - 1) as u32;
                        self.set_of[at] = Some(set);
                        set
                    }
                };
                self.push(SYMBOLS, set, 0);
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
                        let split = self.push(SPLIT, 0, 0);
                        self.steps[split][1] = self.here();
                        self.emit(*arm);
                        jumps.push(self.push(JUMP, 0, 0));
                        self.steps[split][2] = self.here();
                    } else {
                        self.emit(*arm);
                    }
                }
                let end = self.here();
                for jump in jumps {
                    self.steps[jump][1] = end;
                }
            }
            Part::Repeated { what, least, most } => {
                for _ in 0..*least {
                    self.emit(*what);
                }
                match most {
                    None => {
                        let split = self.push(SPLIT, 0, 0);
                        self.steps[split][1] = self.here();
                        self.emit(*what);
                        self.push(JUMP, split as u32, 0);
                        self.steps[split][2] = self.here();
                    }
                    Some(most) => {
                        let mut splits = Vec::new();
                        for _ in *least..*most {
                            let split = self.push(SPLIT, 0, 0);
                            self.steps[split][1] = self.here();
                            splits.push(split);
                            self.emit(*what);
                        }
                        let end = self.here();
                        for split in splits {
                            self.steps[split][2] = end;
                        }
                    }
                }
            }
        }
    }

    /// The machine as words, with each step reading a set pointed at where the set is written.
    fn words(self) -> Vec<u32> {
        let steps = self.steps.len();
        let mut words = Vec::with_capacity(FIRST_STEP + STEP * steps);
        words.push(0);
        words.push(steps as u32);
        let mut sets_at = Vec::with_capacity(self.sets.len());
        let mut at = FIRST_STEP + STEP * steps;
        for set in &self.sets {
            sets_at.push(at as u32);
            at += 1 + 2 * set.len();
        }
        for mut step in self.steps {
            if step[0] == SYMBOLS {
                step[1] = sets_at[step[1] as usize];
            }
            words.extend_from_slice(&step);
        }
        for set in self.sets {
            words.push(set.len() as u32);
            for (from, to) in set {
                words.push(*from);
                words.push(*to);
            }
        }
        words[0] = words.len() as u32;
        words
    }
}

/// How many words a machine is, read off its first word.
pub fn length(first: u32) -> usize {
    first as usize
}

/// Whether the whole of the text is a string the machine accepts.
///
/// # Panics
///
/// Where `machine` is not words [`compile`] answered: a step or a set pointed at past the end.
pub fn matches(machine: &[u32], text: &[u8]) -> bool {
    let steps = machine[1] as usize;
    let step = |at: usize| -> [u32; STEP] {
        let from = FIRST_STEP + STEP * at;
        [machine[from], machine[from + 1], machine[from + 2]]
    };
    let holds = |set: usize, point: u32| -> bool {
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
    };
    // Which steps a character is read at now, which after it, and at which character each step was
    // last added, so that a step is added once per character however many ways reach it.
    let mut now: Vec<usize> = Vec::new();
    let mut next: Vec<usize> = Vec::new();
    let mut added = vec![usize::MAX; steps];
    let mut pending: Vec<usize> = Vec::new();
    let mut add =
        |into: &mut Vec<usize>, from: usize, generation: usize, added: &mut Vec<usize>| {
            pending.push(from);
            while let Some(at) = pending.pop() {
                if added[at] == generation {
                    continue;
                }
                added[at] = generation;
                let [what, one, other] = step(at);
                match what {
                    JUMP => pending.push(one as usize),
                    SPLIT => {
                        pending.push(other as usize);
                        pending.push(one as usize);
                    }
                    FAIL => {}
                    _ => into.push(at),
                }
            }
        };
    add(&mut now, 0, 0, &mut added);
    for (read, point) in scalar_values(text).enumerate() {
        next.clear();
        for at in now.iter().copied() {
            let [what, set, _] = step(at);
            if what == SYMBOLS && holds(set as usize, point) {
                add(&mut next, at + 1, read + 1, &mut added);
            }
        }
        core::mem::swap(&mut now, &mut next);
        if now.is_empty() {
            return false;
        }
    }
    now.iter().any(|at| step(*at)[0] == MATCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(point: char) -> Part {
        Part::Symbols(vec![(point as u32, point as u32)])
    }

    fn digit() -> Part {
        Part::Symbols(vec![(0x30, 0x39)])
    }

    fn accepts(parts: &[Part], text: &str) -> bool {
        matches(&compile(parts).expect("a machine"), text.as_bytes())
    }

    /// `[0-9]{3}-[0-9]{4}`.
    #[test]
    fn a_postal_code_is_matched_whole() {
        let parts = [
            digit(),
            Part::Repeated {
                what: 0,
                least: 3,
                most: Some(3),
            },
            one('-'),
            Part::Repeated {
                what: 0,
                least: 4,
                most: Some(4),
            },
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
            Part::Repeated {
                what: 4,
                least: 0,
                most: None,
            },
            one('𠮷'),
            Part::Repeated {
                what: 6,
                least: 0,
                most: Some(1),
            },
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
        let parts = [
            one('a'),
            Part::Repeated {
                what: 0,
                least: 0,
                most: None,
            },
            Part::Repeated {
                what: 1,
                least: 0,
                most: None,
            },
        ];
        assert!(accepts(&parts, ""));
        assert!(accepts(&parts, "aaaa"));
        assert!(!accepts(&parts, "ab"));
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
        assert_eq!(compile(&[]), Err(Refused::NotAReading));
        assert_eq!(compile(&[Part::InTurn(vec![0])]), Err(Refused::NotAReading));
        assert_eq!(
            compile(&[Part::Symbols(vec![(0xd000, 0xe000)])]),
            Err(Refused::NotAReading)
        );
        assert_eq!(
            compile(&[Part::Symbols(vec![(5, 9), (10, 12)])]),
            Err(Refused::NotAReading)
        );
        assert_eq!(
            compile(&[
                one('a'),
                Part::Repeated {
                    what: 0,
                    least: 3,
                    most: Some(2)
                }
            ]),
            Err(Refused::NotAReading)
        );
    }

    /// A part repeated is read against one set however many times it is repeated.
    #[test]
    fn a_count_repeats_a_part_and_not_its_set() {
        let parts = [
            digit(),
            Part::Repeated {
                what: 0,
                least: 100,
                most: Some(100),
            },
        ];
        let machine = compile(&parts).expect("a machine");
        let steps = machine[1] as usize;
        assert_eq!(machine.len(), FIRST_STEP + STEP * steps + 3);
        assert_eq!(length(machine[0]), machine.len());
    }

    /// Counts inside counts multiply, and past the bound the machine is not built.
    #[test]
    fn a_machine_past_the_bound_is_refused() {
        let parts = [
            one('a'),
            Part::Repeated {
                what: 0,
                least: 1000,
                most: Some(1000),
            },
            Part::Repeated {
                what: 1,
                least: 1000,
                most: Some(1000),
            },
            Part::Repeated {
                what: 2,
                least: 1000,
                most: Some(1000),
            },
        ];
        assert_eq!(compile(&parts), Err(Refused::TooLarge));
    }
}
