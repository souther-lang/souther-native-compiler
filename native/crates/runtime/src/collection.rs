//! A `Set` and a `Map`, as a persistent hash trie kept in the arena.
//!
//! Nothing outside this file reads behind one. Generated code holds the address of a set or a map
//! and calls the functions here, handing over what the runtime cannot know for itself: what an
//! element hashes to and what it is equal to, which are the language's and differ by type
//! (`souther_native_abi::SET_EMPTY`). So how one is kept is decided here alone, and can change
//! without an object noticing.
//!
//! A set and a map are one trie. An entry holds the hash its key was put in under, the key, and a
//! value, which a set leaves at nought. A node takes five bits of the hash a level and holds, for
//! each that some entry below it has, either that entry or the node below; where the bits run out,
//! the entries that share every bit are kept side by side in a node of their own and told apart by
//! equality. Every operation copies the path it changes and shares the rest, since a value is never
//! changed once made and nothing here is ever freed on its own: the arena drops a run's values in
//! one go (`souther_scope_close`).
//!
//! A key the collection holds is never replaced by one equal to it. Two keys equal under the
//! language's equality can still be told apart (`1.0` and `1.00` are one `Decimal` key, written at
//! two scales), and which of them a collection holds is what `Map.keys` and `Set.toList` answer. So
//! the one a collection already holds stays: `Map.insert` under an equal key takes the new value
//! and keeps the key, as the JVM's map does, and `Set.insert` of an equal member leaves the set as
//! it is. Nothing that puts an entry in can write the arriving key where an equal one stands
//! ([`OnEqual`]).
//!
//! What an entry is kept under is the hash it was put in with, and it is never asked for again: a
//! set built as a `Set<A>` and read as a `Set<S>` has the hashes it was built with, which
//! `souther_native_abi::HASHING` holds to be what `S` hashes each of them to. The hash of the key
//! asked for is worked out under the type at the site that asks.

use crate::kernels::{answered, list_of};
use crate::{Bool, Count, List, souther_alloc};
use souther_native_abi::{LIST_LENGTH, NOTHING, list_at, member_at, room_for_members};

/// A set, as the functions here take and answer one: an address only this file reads behind.
#[repr(C)]
pub struct Set {
    _opaque: [u8; 0],
}

/// A map, as the functions here take and answer one: an address only this file reads behind.
#[repr(C)]
pub struct Map {
    _opaque: [u8; 0],
}

/// The slot a value is held at, which is what an optional holding it is: `Map.get`'s answer.
#[repr(C)]
pub struct HeldAt {
    _opaque: [u8; 0],
}

/// A value's hash (`souther_native_abi::HASHING`).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hash(pub i64);

/// What a value hashes to, as generated code works it out for the type at the site that asks: a
/// value as it stands in a slot.
pub type Hasher = unsafe extern "C" fn(i64) -> Hash;

/// Whether two values are equal, as generated code works it out for the type at the site that
/// asks: nought or one.
pub type Equality = unsafe extern "C" fn(i64, i64) -> Bool;

/// How many members a set holds, and how many keys a map: the JVM's number, what a
/// `java.util.Set` or `Map` counts its size in, as a list's bound is (`lists::LIST_HOLDS` in the
/// driver). An insertion or a union past it has no place and ends the run (upstream
/// `KernelContracts`); one from a list never does, since no list holds more.
const HOLDS: i64 = i32::MAX as i64;

/// How many bits of the hash a level of the trie takes.
const BITS: u32 = 5;

/// Where the bits run out: a node this deep keeps its entries side by side.
const DEEPEST: u32 = 64;

/// Marks a word of a node as the address of the node below, where it is not an entry's.
const BELOW: u64 = 1;

/// Marks a node's first word as that of the entries that share every bit of their hash, with how
/// many there are below it, where it is not the bits a node has.
const SIDE_BY_SIDE: u64 = 1 << 63;

/// The collection itself: how many entries it holds, and the trie's top node, or none.
#[derive(Clone, Copy)]
struct Trie {
    size: i64,
    root: u64,
}

/// One entry: the hash it was put in under, the key, and the value.
#[derive(Clone, Copy)]
struct Entry {
    hash: u64,
    key: i64,
    value: i64,
}

/// Room for `words` words, from the arena.
fn room(words: usize) -> *mut u64 {
    souther_alloc(Count(8 * words as i64)).cast()
}

impl Trie {
    const EMPTY: Trie = Trie { size: 0, root: 0 };

    /// The collection at `at`.
    ///
    /// # Safety
    ///
    /// `at` is a set or a map this file answered, and the scope it was made in is still open.
    unsafe fn at<T>(at: *const T) -> Trie {
        let words = at.cast::<u64>();
        unsafe {
            Trie {
                size: words.read() as i64,
                root: words.add(1).read(),
            }
        }
    }

    /// The collection, written into the arena.
    fn kept<T>(self) -> *mut T {
        let at = room(2);
        unsafe {
            at.write(self.size as u64);
            at.add(1).write(self.root);
        }
        at.cast()
    }

    /// The entry under a key equal to `key`, whose hash is `hash`.
    ///
    /// # Safety
    ///
    /// The trie's nodes are ones this file wrote, and `equal` takes two values of the key's type.
    unsafe fn find(&self, hash: u64, key: i64, equal: Equality) -> Option<*const u64> {
        let mut node = self.root;
        let mut shift = 0;
        while node != 0 {
            let at = node as *const u64;
            let header = unsafe { at.read() };
            if header & SIDE_BY_SIDE != 0 {
                let count = (header & !SIDE_BY_SIDE) as usize;
                return (0..count)
                    .map(|index| unsafe { at.add(1 + index).read() } as *const u64)
                    .find(|entry| unsafe { same(*entry, hash, key, equal) });
            }
            let bit = bit_of(hash, shift);
            if header & bit == 0 {
                return None;
            }
            let word = unsafe { at.add(1 + index_of(header, bit)).read() };
            if word & BELOW != 0 {
                node = word & !BELOW;
                shift += BITS;
            } else {
                let entry = word as *const u64;
                return unsafe { same(entry, hash, key, equal) }.then_some(entry);
            }
        }
        None
    }

    /// Every entry, in the trie's order.
    ///
    /// # Safety
    ///
    /// As [`Trie::find`].
    unsafe fn entries(&self) -> Vec<Entry> {
        let mut found = Vec::with_capacity(self.size as usize);
        if self.root != 0 {
            unsafe { gather(self.root, &mut found) };
        }
        found
    }

    /// This collection with `entry` put in, or, where it holds an entry under an equal key, that
    /// entry as `on_equal` says.
    ///
    /// # Safety
    ///
    /// As [`Trie::find`].
    unsafe fn with(&self, entry: Entry, equal: Equality, on_equal: OnEqual) -> Trie {
        match unsafe { put(self.root, 0, entry, equal, on_equal) } {
            Put::Kept => *self,
            Put::Replaced(root) => Trie {
                size: self.size,
                root,
            },
            Put::Added(root) => Trie {
                size: self.size + 1,
                root,
            },
        }
    }

    /// This collection without the entry under a key equal to `key`.
    ///
    /// # Safety
    ///
    /// As [`Trie::find`].
    unsafe fn without(&self, hash: u64, key: i64, equal: Equality) -> Trie {
        if self.root == 0 {
            return *self;
        }
        match unsafe { taken(self.root, 0, hash, key, equal) } {
            None => *self,
            Some(root) => Trie {
                size: self.size - 1,
                root,
            },
        }
    }
}

/// Whether the entry at `entry` is under a key equal to `key`.
///
/// # Safety
///
/// `entry` is an entry this file wrote, and `equal` takes two values of the key's type.
unsafe fn same(entry: *const u64, hash: u64, key: i64, equal: Equality) -> bool {
    unsafe { entry.read() == hash && equal(entry.add(1).read() as i64, key) != Bool::FALSE }
}

/// The entry at `entry`.
///
/// # Safety
///
/// As [`same`].
unsafe fn read_entry(entry: *const u64) -> Entry {
    unsafe {
        Entry {
            hash: entry.read(),
            key: entry.add(1).read() as i64,
            value: entry.add(2).read() as i64,
        }
    }
}

/// `entry`, written into the arena, as the word a node holds it by.
fn write_entry(entry: Entry) -> u64 {
    let at = room(3);
    unsafe {
        at.write(entry.hash);
        at.add(1).write(entry.key as u64);
        at.add(2).write(entry.value as u64);
    }
    at as u64
}

/// The bit of a node's header that the five bits of `hash` at `shift` stand for.
fn bit_of(hash: u64, shift: u32) -> u64 {
    1 << ((hash >> shift) & ((1 << BITS) - 1))
}

/// Where among a node's words the one for `bit` stands: after the ones for every bit below it.
fn index_of(header: u64, bit: u64) -> usize {
    (header & (bit - 1)).count_ones() as usize
}

/// A node with this header and these words, written into the arena, as the word the node above
/// holds it by.
fn write_node(header: u64, words: &[u64]) -> u64 {
    let at = room(1 + words.len());
    unsafe {
        at.write(header);
        for (index, word) in words.iter().enumerate() {
            at.add(1 + index).write(*word);
        }
    }
    at as u64 | BELOW
}

/// The words of the node at `at`, after its header.
///
/// # Safety
///
/// `at` is a node this file wrote, whose header is `header`.
unsafe fn words_of(at: *const u64, header: u64) -> Vec<u64> {
    let count = if header & SIDE_BY_SIDE != 0 {
        (header & !SIDE_BY_SIDE) as usize
    } else {
        header.count_ones() as usize
    };
    (0..count)
        .map(|index| unsafe { at.add(1 + index).read() })
        .collect()
}

/// What putting an entry into a node came to.
enum Put {
    /// An entry under an equal key was there and is kept: the node is the one it was.
    Kept,
    /// An entry under an equal key was there and holds the arriving value now: the new node.
    Replaced(u64),
    /// No entry was under an equal key: the new node, holding one entry more.
    Added(u64),
}

/// What an entry arriving under a key equal to one the collection holds does to the entry there.
/// Neither writes the arriving key: the key a collection holds is the one it keeps.
#[derive(Clone, Copy)]
enum OnEqual {
    /// Nothing: a member equal to one a set holds is that member already.
    Keep,
    /// The entry there holds the arriving value, under the key it held: a map's insertion.
    TakeValue,
}

/// What the entry at `word`, under a key equal to `arriving`'s, becomes as `on_equal` says: the
/// word of an entry of its own hash and key and the arriving value, or none where it is kept. The
/// one place an entry an equal key arrives at is written, so no other can write the arriving key
/// there.
///
/// # Safety
///
/// `word` is an entry this file wrote.
unsafe fn arrived_at(word: u64, arriving: &Entry, on_equal: OnEqual) -> Option<u64> {
    match on_equal {
        OnEqual::Keep => None,
        OnEqual::TakeValue => {
            let there = unsafe { read_entry(word as *const u64) };
            Some(write_entry(Entry {
                value: arriving.value,
                ..there
            }))
        }
    }
}

/// The node at `node` (none where it is nought), `shift` bits down, with `entry` put in.
///
/// # Safety
///
/// As [`Trie::find`].
unsafe fn put(node: u64, shift: u32, entry: Entry, equal: Equality, on_equal: OnEqual) -> Put {
    if node == 0 {
        return Put::Added(write_node(bit_of(entry.hash, shift), &[write_entry(entry)]) & !BELOW);
    }
    let at = (node & !BELOW) as *const u64;
    let header = unsafe { at.read() };
    let mut words = unsafe { words_of(at, header) };
    if header & SIDE_BY_SIDE != 0 {
        let equal_at = words
            .iter()
            .position(|word| unsafe { same(*word as *const u64, entry.hash, entry.key, equal) });
        return match equal_at {
            Some(index) => match unsafe { arrived_at(words[index], &entry, on_equal) } {
                None => Put::Kept,
                Some(arrived) => {
                    words[index] = arrived;
                    Put::Replaced(write_node(header, &words) & !BELOW)
                }
            },
            None => {
                words.push(write_entry(entry));
                Put::Added(write_node(SIDE_BY_SIDE | words.len() as u64, &words) & !BELOW)
            }
        };
    }
    let bit = bit_of(entry.hash, shift);
    let index = index_of(header, bit);
    if header & bit == 0 {
        words.insert(index, write_entry(entry));
        return Put::Added(write_node(header | bit, &words) & !BELOW);
    }
    let word = words[index];
    if word & BELOW != 0 {
        return match unsafe { put(word & !BELOW, shift + BITS, entry, equal, on_equal) } {
            Put::Kept => Put::Kept,
            Put::Replaced(below) => {
                words[index] = below | BELOW;
                Put::Replaced(write_node(header, &words) & !BELOW)
            }
            Put::Added(below) => {
                words[index] = below | BELOW;
                Put::Added(write_node(header, &words) & !BELOW)
            }
        };
    }
    let there = unsafe { read_entry(word as *const u64) };
    if there.hash == entry.hash && unsafe { equal(there.key, entry.key) } != Bool::FALSE {
        return match unsafe { arrived_at(word, &entry, on_equal) } {
            None => Put::Kept,
            Some(arrived) => {
                words[index] = arrived;
                Put::Replaced(write_node(header, &words) & !BELOW)
            }
        };
    }
    words[index] = apart(
        shift + BITS,
        word,
        there.hash,
        write_entry(entry),
        entry.hash,
    );
    Put::Added(write_node(header, &words) & !BELOW)
}

/// A node `shift` bits down holding two entries under unequal keys, as the word the node above
/// holds it by.
fn apart(shift: u32, one: u64, one_hash: u64, other: u64, other_hash: u64) -> u64 {
    if shift >= DEEPEST {
        return write_node(SIDE_BY_SIDE | 2, &[one, other]);
    }
    let one_bit = bit_of(one_hash, shift);
    let other_bit = bit_of(other_hash, shift);
    if one_bit == other_bit {
        write_node(
            one_bit,
            &[apart(shift + BITS, one, one_hash, other, other_hash)],
        )
    } else if one_bit < other_bit {
        write_node(one_bit | other_bit, &[one, other])
    } else {
        write_node(one_bit | other_bit, &[other, one])
    }
}

/// The node at `node`, `shift` bits down, without the entry under a key equal to `key`: nought
/// where that leaves it empty, and none where no such entry is there.
///
/// # Safety
///
/// As [`Trie::find`].
unsafe fn taken(node: u64, shift: u32, hash: u64, key: i64, equal: Equality) -> Option<u64> {
    let at = node as *const u64;
    let header = unsafe { at.read() };
    let mut words = unsafe { words_of(at, header) };
    if header & SIDE_BY_SIDE != 0 {
        let index = words
            .iter()
            .position(|word| unsafe { same(*word as *const u64, hash, key, equal) })?;
        words.remove(index);
        return Some(if words.is_empty() {
            0
        } else {
            write_node(SIDE_BY_SIDE | words.len() as u64, &words) & !BELOW
        });
    }
    let bit = bit_of(hash, shift);
    if header & bit == 0 {
        return None;
    }
    let index = index_of(header, bit);
    let word = words[index];
    let left = if word & BELOW != 0 {
        unsafe { taken(word & !BELOW, shift + BITS, hash, key, equal) }?
    } else if unsafe { same(word as *const u64, hash, key, equal) } {
        0
    } else {
        return None;
    };
    if left == 0 {
        words.remove(index);
        let header = header & !bit;
        return Some(if header == 0 {
            0
        } else {
            write_node(header, &words) & !BELOW
        });
    }
    words[index] = left | BELOW;
    Some(write_node(header, &words) & !BELOW)
}

/// Every entry below the node at `node`, in the trie's order, onto `found`.
///
/// # Safety
///
/// `node` is a node this file wrote, and the scope it was made in is still open.
unsafe fn gather(node: u64, found: &mut Vec<Entry>) {
    let at = node as *const u64;
    let header = unsafe { at.read() };
    for word in unsafe { words_of(at, header) } {
        if word & BELOW != 0 {
            unsafe { gather(word & !BELOW, found) };
        } else {
            found.push(unsafe { read_entry(word as *const u64) });
        }
    }
}

/// Where the functions below are handed an element, as the entry a set keeps it in.
///
/// # Safety
///
/// `hash` hashes a value of the element's type.
unsafe fn member(element: i64, hash: Hasher) -> Entry {
    Entry {
        hash: unsafe { hash(element) }.0 as u64,
        key: element,
        value: 0,
    }
}

/// The empty set.
#[unsafe(no_mangle)]
pub extern "C" fn souther_set_empty() -> *mut Set {
    Trie::EMPTY.kept()
}

/// `Set.insert`, written through `out` where the set it makes holds no more members than a set
/// holds: the set itself where it holds a member equal to `element`, which it keeps.
///
/// # Safety
///
/// `set` is a set this file answered, the scope it was made in is still open, and `hash` and `equal` take
/// values of its elements' type. So for every function here that takes a set. And `out` is room
/// for the address of one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_insert(
    set: *const Set,
    element: i64,
    hash: Hasher,
    equal: Equality,
    out: *mut *const Set,
) -> Bool {
    let trie = unsafe { Trie::at(set) };
    let entry = unsafe { member(element, hash) };
    let with = unsafe { trie.with(entry, equal, OnEqual::Keep) };
    let made = if with.size == trie.size {
        Some(set)
    } else {
        held(with)
    };
    unsafe { answered(made, out) }
}

/// The collection written into the arena, where it holds no more than a set or a map holds.
fn held<T>(trie: Trie) -> Option<*const T> {
    (trie.size <= HOLDS).then(|| trie.kept::<T>().cast_const())
}

/// `Set.remove`: the set itself where it holds no member equal to `element`.
///
/// # Safety
///
/// As [`souther_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_remove(
    set: *const Set,
    element: i64,
    hash: Hasher,
    equal: Equality,
) -> *const Set {
    let trie = unsafe { Trie::at(set) };
    let without = unsafe { trie.without(hash(element).0 as u64, element, equal) };
    if without.size == trie.size {
        set
    } else {
        without.kept()
    }
}

/// `Set.contains`.
///
/// # Safety
///
/// As [`souther_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_contains(
    set: *const Set,
    element: i64,
    hash: Hasher,
    equal: Equality,
) -> Bool {
    let trie = unsafe { Trie::at(set) };
    unsafe { trie.find(hash(element).0 as u64, element, equal) }
        .is_some()
        .into()
}

/// `Set.union`: every member of either, written through `out` where that is no more members than a
/// set holds. The smaller set's members are put into the larger, `a` counting as the larger where
/// the two are as large, so where both hold a member the larger's is the one kept: which member a
/// set keeps of two equal ones is the language's to leave open (spec §stdlib-set), and this is the
/// one the JVM's `PersistentHashSet.union` keeps.
///
/// # Safety
///
/// As [`souther_set_insert`], for both sets.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_union(
    a: *const Set,
    b: *const Set,
    equal: Equality,
    out: *mut *const Set,
) -> Bool {
    let (larger, smaller) = unsafe { larger_first(a, b) };
    let mut union = larger;
    for entry in unsafe { smaller.entries() } {
        union = unsafe { union.with(entry, equal, OnEqual::Keep) };
    }
    unsafe { answered(held(union), out) }
}

/// `Set.intersection`: the members both hold. The smaller set's are walked and kept where the larger
/// holds one equal, `a` counting as the larger where the two are as large, so the member kept is the
/// smaller's, as the JVM's `PersistentHashSet.intersect` keeps it.
///
/// # Safety
///
/// As [`souther_set_union`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_intersection(
    a: *const Set,
    b: *const Set,
    equal: Equality,
) -> *const Set {
    let (larger, smaller) = unsafe { larger_first(a, b) };
    let mut kept = Trie::EMPTY;
    for entry in unsafe { smaller.entries() } {
        if unsafe { larger.find(entry.hash, entry.key, equal) }.is_some() {
            kept = unsafe { kept.with(entry, equal, OnEqual::Keep) };
        }
    }
    kept.kept()
}

/// `Set.difference`: `a`'s members that `b` holds none equal to.
///
/// # Safety
///
/// As [`souther_set_union`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_difference(
    a: *const Set,
    b: *const Set,
    equal: Equality,
) -> *const Set {
    let (a, b) = unsafe { (Trie::at(a), Trie::at(b)) };
    let mut kept = Trie::EMPTY;
    for entry in unsafe { a.entries() } {
        if unsafe { b.find(entry.hash, entry.key, equal) }.is_none() {
            kept = unsafe { kept.with(entry, equal, OnEqual::Keep) };
        }
    }
    kept.kept()
}

/// The two sets, the one holding more first, and `a` first where they hold as many.
///
/// # Safety
///
/// As [`souther_set_union`].
unsafe fn larger_first(a: *const Set, b: *const Set) -> (Trie, Trie) {
    let (a, b) = unsafe { (Trie::at(a), Trie::at(b)) };
    if a.size >= b.size { (a, b) } else { (b, a) }
}

/// `Set.size`.
///
/// # Safety
///
/// As [`souther_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_size(set: *const Set) -> i64 {
    unsafe { Trie::at(set) }.size
}

/// `Set.toList`, in the trie's order.
///
/// # Safety
///
/// As [`souther_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_to_list(set: *const Set) -> *mut List {
    list_of(&unsafe { Trie::at(set).entries() }, |entry| entry.key)
}

/// `Set.fromList`: of elements that are equal, the first is kept.
///
/// # Safety
///
/// `list` is a list whose elements are of the type `hash` and `equal` take, and the mark below it
/// still stands.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_from_list(
    list: *const List,
    hash: Hasher,
    equal: Equality,
) -> *const Set {
    let mut set = Trie::EMPTY;
    for element in unsafe { elements(list) } {
        set = unsafe { set.with(member(element, hash), equal, OnEqual::Keep) };
    }
    set.kept()
}

/// The elements `list` holds more than once, each once, in the order its repetition was found:
/// what Raoh's `unique` reports as `duplicates`, and nothing where every element is its own.
///
/// # Safety
///
/// As [`souther_set_from_list`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_list_duplicates(
    list: *const List,
    hash: Hasher,
    equal: Equality,
) -> *const List {
    let mut seen = Trie::EMPTY;
    let mut again = Trie::EMPTY;
    let mut repeated = Vec::new();
    for element in unsafe { elements(list) } {
        let entry = unsafe { member(element, hash) };
        if unsafe { seen.find(entry.hash, element, equal) }.is_none() {
            seen = unsafe { seen.with(entry, equal, OnEqual::Keep) };
        } else if unsafe { again.find(entry.hash, element, equal) }.is_none() {
            again = unsafe { again.with(entry, equal, OnEqual::Keep) };
            repeated.push(element);
        }
    }
    crate::kernels::list_of(&repeated, |it| *it)
}

/// Whether two sets hold equal members: as many, and each of `a`'s equal to one of `b`'s. The
/// order either keeps them in is not asked.
///
/// # Safety
///
/// As [`souther_set_union`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_equal(a: *const Set, b: *const Set, equal: Equality) -> Bool {
    let (a, b) = unsafe { (Trie::at(a), Trie::at(b)) };
    let same = a.size == b.size
        && unsafe { a.entries() }
            .iter()
            .all(|entry| unsafe { b.find(entry.hash, entry.key, equal) }.is_some());
    same.into()
}

/// A set's hash: its size, with the sum of what each member's kept hash mixes to, which is the
/// same whatever order the members are kept in.
///
/// # Safety
///
/// As [`souther_set_insert`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_set_hash(set: *const Set) -> Hash {
    let trie = unsafe { Trie::at(set) };
    let sum = unsafe { trie.entries() }
        .iter()
        .fold(0u64, |sum, entry| sum.wrapping_add(mix(entry.hash)));
    souther_hash_combine(Hash(trie.size), sum as i64)
}

/// The empty map.
#[unsafe(no_mangle)]
pub extern "C" fn souther_map_empty() -> *mut Map {
    Trie::EMPTY.kept()
}

/// `Map.get`: the slot the value under a key equal to `key` is held at, which is what an optional
/// of it is, or [`NOTHING`].
///
/// # Safety
///
/// `map` is a map this file answered, the scope it was made in is still open, and `hash` and `equal` take
/// values of its keys' type. So for every function here that takes a map.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_get(
    map: *const Map,
    key: i64,
    hash: Hasher,
    equal: Equality,
) -> *const HeldAt {
    let trie = unsafe { Trie::at(map) };
    match unsafe { trie.find(hash(key).0 as u64, key, equal) } {
        Some(entry) => unsafe { entry.add(2) }.cast(),
        None => NOTHING as *const HeldAt,
    }
}

/// `Map.containsKey`.
///
/// # Safety
///
/// As [`souther_map_get`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_contains_key(
    map: *const Map,
    key: i64,
    hash: Hasher,
    equal: Equality,
) -> Bool {
    let trie = unsafe { Trie::at(map) };
    unsafe { trie.find(hash(key).0 as u64, key, equal) }
        .is_some()
        .into()
}

/// `Map.keys`, in the trie's order.
///
/// # Safety
///
/// As [`souther_map_get`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_keys(map: *const Map) -> *mut List {
    list_of(&unsafe { Trie::at(map).entries() }, |entry| entry.key)
}

/// `Map.values`, in the order [`souther_map_keys`] lists their keys.
///
/// # Safety
///
/// As [`souther_map_get`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_values(map: *const Map) -> *mut List {
    list_of(&unsafe { Trie::at(map).entries() }, |entry| entry.value)
}

/// `Map.insert`: `value` under `key`, in place of what a key equal to it held, which stays the key
/// the map holds, written through `out` where that is no more keys than a map holds.
///
/// # Safety
///
/// As [`souther_map_get`], and `out` is room for the address of a map.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_insert(
    map: *const Map,
    key: i64,
    value: i64,
    hash: Hasher,
    equal: Equality,
    out: *mut *const Map,
) -> Bool {
    let trie = unsafe { Trie::at(map) };
    let entry = Entry {
        hash: unsafe { hash(key) }.0 as u64,
        key,
        value,
    };
    unsafe { answered(held(trie.with(entry, equal, OnEqual::TakeValue)), out) }
}

/// `Map.remove`: the map itself where it holds no key equal to `key`.
///
/// # Safety
///
/// As [`souther_map_get`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_remove(
    map: *const Map,
    key: i64,
    hash: Hasher,
    equal: Equality,
) -> *const Map {
    let trie = unsafe { Trie::at(map) };
    let without = unsafe { trie.without(hash(key).0 as u64, key, equal) };
    if without.size == trie.size {
        map
    } else {
        without.kept()
    }
}

/// `Map.size`.
///
/// # Safety
///
/// As [`souther_map_get`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_size(map: *const Map) -> i64 {
    unsafe { Trie::at(map) }.size
}

/// `Map.toList`: a pair for each entry, the key and then the value, in the order
/// [`souther_map_keys`] lists the keys.
///
/// # Safety
///
/// As [`souther_map_get`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_to_list(map: *const Map) -> *mut List {
    list_of(&unsafe { Trie::at(map).entries() }, |entry| {
        let pair = souther_alloc(Count(room_for_members(2)));
        unsafe {
            pair.offset(member_at(0) as isize)
                .cast::<i64>()
                .write(entry.key);
            pair.offset(member_at(1) as isize)
                .cast::<i64>()
                .write(entry.value);
        }
        pair as i64
    })
}

/// `Map.fromList`: a list of pairs, a later pair's value winning where two keys are equal, under
/// the earlier pair's key.
///
/// # Safety
///
/// `list` is a list of pairs whose keys are of the type `hash` and `equal` take, and the mark below
/// it still stands.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_from_list(
    list: *const List,
    hash: Hasher,
    equal: Equality,
) -> *mut Map {
    let mut map = Trie::EMPTY;
    for pair in unsafe { elements(list) } {
        let pair = pair as *const u8;
        let (key, value) = unsafe {
            (
                pair.offset(member_at(0) as isize).cast::<i64>().read(),
                pair.offset(member_at(1) as isize).cast::<i64>().read(),
            )
        };
        let entry = Entry {
            hash: unsafe { hash(key) }.0 as u64,
            key,
            value,
        };
        map = unsafe { map.with(entry, equal, OnEqual::TakeValue) };
    }
    map.kept()
}

/// Whether two maps hold equal values under equal keys: as many keys, and under each of `a`'s a
/// value equal to what `b` holds under an equal key.
///
/// # Safety
///
/// As [`souther_map_get`], for both maps, and `values` takes two values of their values' type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_equal(
    a: *const Map,
    b: *const Map,
    keys: Equality,
    values: Equality,
) -> Bool {
    let (a, b) = unsafe { (Trie::at(a), Trie::at(b)) };
    let same = a.size == b.size
        && unsafe { a.entries() }.iter().all(|entry| {
            unsafe { b.find(entry.hash, entry.key, keys) }.is_some_and(|there| unsafe {
                values(entry.value, read_entry(there).value) != Bool::FALSE
            })
        });
    same.into()
}

/// A map's hash: its size, with the sum over its entries of what each key's kept hash and its
/// value's hash combine to, which is the same whatever order the entries are kept in.
///
/// # Safety
///
/// As [`souther_map_get`], and `values` hashes a value of the map's values' type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_map_hash(map: *const Map, values: Hasher) -> Hash {
    let trie = unsafe { Trie::at(map) };
    let sum = unsafe { trie.entries() }.iter().fold(0u64, |sum, entry| {
        let value = unsafe { values(entry.value) };
        sum.wrapping_add(souther_hash_combine(Hash(entry.hash as i64), value.0).0 as u64)
    });
    souther_hash_combine(Hash(trie.size), sum as i64)
}

/// The elements of a list, as the words its slots hold.
///
/// # Safety
///
/// `list` is a list, and the scope it was made in is still open.
unsafe fn elements(list: *const List) -> Vec<i64> {
    let at = list.cast::<u8>();
    let length = unsafe { at.offset(LIST_LENGTH as isize).cast::<i64>().read() };
    (0..length)
        .map(|index| unsafe { at.offset(list_at(index) as isize).cast::<i64>().read() })
        .collect()
}

/// Sixty-four bits spread over sixty-four: the finalizer of SplitMix64, so that two words that
/// differ in one bit differ in about half after.
fn mix(word: u64) -> u64 {
    let mut z = word.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A hash with one more word composed into it, in order: `combine(combine(h, a), b)` is not
/// `combine(combine(h, b), a)`. What every hash generated code works out is composed with
/// (`souther_native_abi::HASHING`).
#[unsafe(no_mangle)]
pub extern "C" fn souther_hash_combine(hash: Hash, word: i64) -> Hash {
    Hash(mix((hash.0 as u64).rotate_left(5) ^ mix(word as u64)) as i64)
}

/// The hash of a run of bytes: FNV-1a over them, and then mixed.
pub(crate) fn hash_of_bytes(bytes: &[u8]) -> Hash {
    hash_of_parts(&[bytes])
}

/// The hash of runs of bytes one after another, each after its length, so that where one ends is
/// part of what is hashed: FNV-1a over them, and then mixed. What a value made of parts is hashed
/// by without writing it out as one run first.
pub(crate) fn hash_of_parts(parts: &[&[u8]]) -> Hash {
    let fold = |hash: u64, byte: &u8| (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    let folded = parts.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, part| {
        let hash = (part.len() as u64).to_le_bytes().iter().fold(hash, fold);
        part.iter().fold(hash, fold)
    });
    Hash(mix(folded) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{souther_scope_close, souther_scope_open};

    /// A key's hash, from what it is: the number, or where it is all but one bit the same as every
    /// other key's, so that every key lands in one node side by side.
    unsafe extern "C" fn spread(key: i64) -> Hash {
        Hash(mix(key as u64) as i64)
    }

    unsafe extern "C" fn clashing(key: i64) -> Hash {
        Hash(key & 1)
    }

    unsafe extern "C" fn numbers(a: i64, b: i64) -> Bool {
        (a == b).into()
    }

    fn set_of(elements: &[i64], hash: Hasher) -> *const Set {
        let list = list_of(elements, |it| *it);
        unsafe { souther_set_from_list(list, hash, numbers) }
    }

    fn inserted(set: *const Set, element: i64, hash: Hasher) -> *const Set {
        let mut out = std::ptr::null();
        assert_eq!(
            unsafe { souther_set_insert(set, element, hash, numbers, &mut out) },
            Bool::TRUE
        );
        out
    }

    fn union(a: *const Set, b: *const Set) -> *const Set {
        let mut out = std::ptr::null();
        assert_eq!(
            unsafe { souther_set_union(a, b, numbers, &mut out) },
            Bool::TRUE
        );
        out
    }

    /// Whether the collection at `at` keeps any entries side by side, which only entries whose
    /// hashes share every bit and whose keys are not equal make. A test meaning to walk those
    /// nodes asks this of what it built, so that data that no longer makes one fails the test
    /// rather than quietly walking the nodes that split.
    fn keeps_side_by_side<T>(at: *const T) -> bool {
        let mut left = vec![unsafe { Trie::at(at) }.root];
        while let Some(node) = left.pop() {
            if node == 0 {
                continue;
            }
            let at = node as *const u64;
            let header = unsafe { at.read() };
            if header & SIDE_BY_SIDE != 0 {
                return true;
            }
            for word in unsafe { words_of(at, header) } {
                if word & BELOW != 0 {
                    left.push(word & !BELOW);
                }
            }
        }
        false
    }

    fn members(set: *const Set) -> Vec<i64> {
        let mut listed = unsafe { elements(souther_set_to_list(set)) };
        listed.sort();
        listed
    }

    /// Every operation, under a hash that spreads and one that sends every key to one place, so
    /// that the nodes that keep entries side by side are walked as well as the ones that split.
    #[test]
    fn a_set_holds_each_member_once_whatever_its_hash() {
        let scope = souther_scope_open();
        for (hash, clashes) in [(spread as Hasher, false), (clashing as Hasher, true)] {
            let set = set_of(&[3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5], hash);
            if clashes {
                assert!(
                    keeps_side_by_side(set),
                    "every odd member hashes to one place"
                );
            }
            assert_eq!(members(set), vec![1, 2, 3, 4, 5, 6, 9]);
            assert_eq!(unsafe { souther_set_size(set) }, 7);
            assert_eq!(
                unsafe { souther_set_contains(set, 9, hash, numbers) },
                Bool::TRUE
            );
            assert_eq!(
                unsafe { souther_set_contains(set, 7, hash, numbers) },
                Bool::FALSE
            );
            let fewer = unsafe { souther_set_remove(set, 4, hash, numbers) };
            assert_eq!(members(fewer), vec![1, 2, 3, 5, 6, 9]);
            assert_eq!(
                members(set),
                vec![1, 2, 3, 4, 5, 6, 9],
                "the set removed from stays"
            );
            assert_eq!(unsafe { souther_set_remove(set, 7, hash, numbers) }, set);
            assert_eq!(inserted(set, 9, hash), set);
            let other = set_of(&[9, 10, 1], hash);
            assert_eq!(members(union(set, other)), vec![1, 2, 3, 4, 5, 6, 9, 10]);
            assert_eq!(
                members(unsafe { souther_set_intersection(set, other, numbers) }),
                vec![1, 9]
            );
            assert_eq!(
                members(unsafe { souther_set_difference(set, other, numbers) }),
                vec![2, 3, 4, 5, 6]
            );
            let emptied = [1, 2, 3, 4, 5, 6, 9].iter().fold(set, |set, it| unsafe {
                souther_set_remove(set, *it, hash, numbers)
            });
            assert_eq!(unsafe { souther_set_size(emptied) }, 0);
            assert_eq!(members(emptied), Vec::<i64>::new());
        }
        souther_scope_close(scope);
    }

    /// Equal where the members are, whatever order they went in: which is what the hash of one
    /// has to agree with.
    #[test]
    fn two_sets_built_apart_are_equal_and_hash_alike_where_their_members_are() {
        let scope = souther_scope_open();
        let many: Vec<i64> = (0..500).collect();
        let backwards: Vec<i64> = many.iter().rev().copied().collect();
        let one = set_of(&many, spread);
        let other = set_of(&backwards, spread);
        assert_eq!(
            unsafe { souther_set_equal(one, other, numbers) },
            Bool::TRUE
        );
        assert_eq!(unsafe { souther_set_hash(one) }, unsafe {
            souther_set_hash(other)
        });
        let fewer = unsafe { souther_set_remove(other, 250, spread, numbers) };
        assert_eq!(
            unsafe { souther_set_equal(one, fewer, numbers) },
            Bool::FALSE
        );
        let swapped = inserted(fewer, 1000, spread);
        assert_eq!(
            unsafe { souther_set_equal(one, swapped, numbers) },
            Bool::FALSE
        );
        souther_scope_close(scope);
    }

    /// A set already holding as many members as a set holds takes no other, and still takes one
    /// equal to one it holds. Its size is read, not counted, so no such set has to be built.
    #[test]
    fn a_set_as_full_as_a_set_holds_takes_no_new_member() {
        let scope = souther_scope_open();
        let full: *const Set = Trie {
            size: HOLDS,
            root: 0,
        }
        .kept();
        let mut out = std::ptr::null();
        assert_eq!(
            unsafe { souther_set_insert(full, 1, spread, numbers, &mut out) },
            Bool::FALSE
        );
        assert!(out.is_null(), "nothing is written where it takes none");
        let one = set_of(&[1], spread);
        assert_eq!(
            unsafe { souther_set_union(full, one, numbers, &mut out) },
            Bool::FALSE
        );
        let full_map: *const Map = Trie {
            size: HOLDS,
            root: 0,
        }
        .kept();
        let mut map_out = std::ptr::null();
        let wrote = unsafe { souther_map_insert(full_map, 1, 1, spread, numbers, &mut map_out) };
        assert_eq!(wrote, Bool::FALSE);
        souther_scope_close(scope);
    }

    /// A `Decimal` hashes as its amount and a `Rational` as the value it is, however each was
    /// reached: `1.0` and `1.00` alike, `1 / 2` and `2 / 4` alike, and neither alike another value.
    #[test]
    fn equal_amounts_and_equal_ratios_hash_alike() {
        use crate::amount::Amount;
        use crate::decimal::{decimal_of, souther_decimal_hash};
        use crate::rational::{Rational, souther_rational_divide, souther_rational_from_int};
        let scope = souther_scope_open();
        let decimal = |unscaled: u8, scale| unsafe {
            souther_decimal_hash(decimal_of(&Amount::from_trusted_parts(
                false,
                &[unscaled],
                scale,
            )))
        };
        assert_eq!(decimal(10, 1), decimal(100, 2));
        assert_ne!(decimal(10, 1), decimal(11, 1));
        assert_ne!(decimal(10, 1), decimal(10, 2));
        let ratio = |top, bottom| unsafe {
            let mut out: *mut Rational = std::ptr::null_mut();
            let wrote = souther_rational_divide(
                souther_rational_from_int(top),
                souther_rational_from_int(bottom),
                &mut out,
            );
            assert_eq!(wrote, Bool::TRUE);
            crate::rational::souther_rational_hash(out)
        };
        assert_eq!(ratio(1, 2), ratio(2, 4));
        assert_ne!(ratio(1, 2), ratio(1, 3));
        assert_ne!(ratio(1, 2), ratio(-1, 2));
        souther_scope_close(scope);
    }

    /// Keys equal where their last digits are, as `1.0` and `1.00` are one `Decimal`: equal, and
    /// told apart by what they are.
    unsafe extern "C" fn by_last_digit(a: i64, b: i64) -> Bool {
        (a.rem_euclid(10) == b.rem_euclid(10)).into()
    }

    unsafe extern "C" fn last_digit(key: i64) -> Hash {
        Hash(mix(key.rem_euclid(10) as u64) as i64)
    }

    /// Every odd key to one place and every even one to another: `1` and `3` share every bit of
    /// their hash and are not equal, so they are kept side by side, and `11` and `13`, equal to them,
    /// arrive there.
    unsafe extern "C" fn last_digit_clashing(key: i64) -> Hash {
        Hash(key.rem_euclid(10) & 1)
    }

    fn map_with(pairs: &[(i64, i64)], hash: Hasher) -> *const Map {
        pairs
            .iter()
            .fold(souther_map_empty().cast_const(), |map, (key, value)| {
                let mut out = std::ptr::null();
                let wrote =
                    unsafe { souther_map_insert(map, *key, *value, hash, by_last_digit, &mut out) };
                assert_eq!(wrote, Bool::TRUE);
                out
            })
    }

    fn sorted_pairs(map: *const Map) -> Vec<(i64, i64)> {
        let mut pairs: Vec<(i64, i64)> = unsafe { elements(souther_map_to_list(map)) }
            .into_iter()
            .map(|pair| {
                let pair = pair as *const i64;
                unsafe { (pair.read(), pair.add(1).read()) }
            })
            .collect();
        pairs.sort();
        pairs
    }

    /// A key a collection holds stays the key it holds when an equal one arrives: a map takes the
    /// arriving value under the key it had, and a set keeps the member it had, whether the two meet
    /// where the trie splits or side by side at its bottom, which the clashing hash is held to reach.
    #[test]
    fn a_key_held_is_not_replaced_by_an_equal_one() {
        let scope = souther_scope_open();
        for (hash, clashing) in [
            (last_digit as Hasher, false),
            (last_digit_clashing as Hasher, true),
        ] {
            let map = map_with(&[(1, 10), (3, 30)], hash);
            if clashing {
                assert!(keeps_side_by_side(map), "1 and 3 hash to one place");
            }
            let map = map_with(&[(1, 10), (3, 30), (11, 110), (13, 130)], hash);
            assert_eq!(
                sorted_pairs(map),
                vec![(1, 110), (3, 130)],
                "the keys first put in, the values last put in"
            );

            let pairs = [(1, 10), (3, 30), (11, 110), (13, 130)].map(|(key, value)| {
                let pair = souther_alloc(Count(room_for_members(2)));
                unsafe {
                    pair.cast::<i64>().write(key);
                    pair.cast::<i64>().add(1).write(value);
                }
                pair as i64
            });
            let listed = list_of(&pairs, |it| *it);
            let built = unsafe { souther_map_from_list(listed, hash, by_last_digit) };
            if clashing {
                assert!(keeps_side_by_side(built), "1 and 3 hash to one place");
            }
            assert_eq!(sorted_pairs(built), vec![(1, 110), (3, 130)]);

            let listed = list_of(&[1, 3, 11, 13], |it| *it);
            let set = unsafe { souther_set_from_list(listed, hash, by_last_digit) };
            if clashing {
                assert!(keeps_side_by_side(set), "1 and 3 hash to one place");
            }
            let mut out = std::ptr::null();
            unsafe { souther_set_insert(set, 21, hash, by_last_digit, &mut out) };
            assert_eq!(members(out), vec![1, 3], "the members first put in");
        }
        souther_scope_close(scope);
    }

    fn map_of(pairs: &[(i64, i64)], hash: Hasher) -> *const Map {
        pairs
            .iter()
            .fold(souther_map_empty().cast_const(), |map, (key, value)| {
                let mut out = std::ptr::null();
                let wrote =
                    unsafe { souther_map_insert(map, *key, *value, hash, numbers, &mut out) };
                assert_eq!(wrote, Bool::TRUE);
                out
            })
    }

    fn got(map: *const Map, key: i64, hash: Hasher) -> Option<i64> {
        let held = unsafe { souther_map_get(map, key, hash, numbers) };
        (held as i64 != NOTHING).then(|| unsafe { held.cast::<i64>().read() })
    }

    #[test]
    fn a_map_holds_the_last_value_put_under_a_key_whatever_its_hash() {
        let scope = souther_scope_open();
        for (hash, clashes) in [(spread as Hasher, false), (clashing as Hasher, true)] {
            let map = map_of(&[(1, 10), (2, 20), (3, 30), (1, 11)], hash);
            if clashes {
                assert!(keeps_side_by_side(map), "1 and 3 hash to one place");
            }
            assert_eq!(unsafe { souther_map_size(map) }, 3);
            assert_eq!(got(map, 1, hash), Some(11));
            assert_eq!(got(map, 4, hash), None);
            let fewer = unsafe { souther_map_remove(map, 2, hash, numbers) };
            assert_eq!(got(fewer, 2, hash), None);
            assert_eq!(got(map, 2, hash), Some(20), "the map removed from stays");
            let mut keys = unsafe { elements(souther_map_keys(map)) };
            keys.sort();
            assert_eq!(keys, vec![1, 2, 3]);
            let pairs = unsafe { souther_map_to_list(map) };
            let back = unsafe { souther_map_from_list(pairs, hash, numbers) };
            assert_eq!(
                unsafe { souther_map_equal(map, back, numbers, numbers) },
                Bool::TRUE
            );
            let other = map_of(&[(1, 11), (2, 20), (3, 31)], hash);
            assert_eq!(
                unsafe { souther_map_equal(map, other, numbers, numbers) },
                Bool::FALSE
            );
            assert_eq!(unsafe { souther_map_hash(map, spread) }, unsafe {
                souther_map_hash(back, spread)
            });
        }
        souther_scope_close(scope);
    }
}
