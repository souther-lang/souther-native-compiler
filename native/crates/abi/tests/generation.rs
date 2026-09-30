//! The generation is held to what it versions.
//!
//! `ABI_GENERATION` is a number somebody moves by hand, on the day a call generated code makes to
//! the runtime, or a call a host makes, or what a status means, stops being what an object built
//! before it expects. Nothing failed when that day came and the number was left where it was, so
//! two objects agreeing on a symbol's name while disagreeing about how a call to it is made linked
//! without a word (souther-native-compiler#107: `souther_string_concat` began to write its answer
//! through room).
//!
//! So each generation has a record of the contract as it was when the generation ended, in
//! `generations/<n>.txt`, one line for each function either side calls, each case a host reads and
//! each number the two sides share. What is held is that nothing the record for the current
//! generation says has changed: a line added is a function no object of that generation calls, and
//! is compatible, and a line changed or gone is a contract that moved and needs the next
//! generation. A generation that moves records the contract it starts from.
//!
//! The lines are read off the tables themselves, and the numbers are the list below, which a scan
//! of the source holds to every number the crate states.

use souther_native_abi::*;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Every number the two sides share, by the name the source gives it.
fn constants() -> Vec<(&'static str, i128)> {
    vec![
        ("NO_FAILED_CLAUSE", NO_FAILED_CLAUSE.into()),
        ("SLOT", SLOT.into()),
        ("WHICH", WHICH.into()),
        ("FIRST_FIELD", FIRST_FIELD.into()),
        ("CARRIED", CARRIED.into()),
        ("LIST_LENGTH", LIST_LENGTH.into()),
        ("LIST_ELEMENTS", LIST_ELEMENTS.into()),
        ("CAPABILITY_INVOKE", CAPABILITY_INVOKE.into()),
        ("CAPABILITY_ENVIRONMENT", CAPABILITY_ENVIRONMENT.into()),
        ("HOSTED_IMPLEMENTATION", HOSTED_IMPLEMENTATION.into()),
        ("HOSTED_USERDATA", HOSTED_USERDATA.into()),
        ("FUNCTION_INVOKE", FUNCTION_INVOKE.into()),
        ("HOSTED_FUNCTION_HOSTED", HOSTED_FUNCTION_HOSTED.into()),
        ("NOTHING", NOTHING.into()),
        ("HELD", HELD.into()),
        ("TEXT_LENGTH", TEXT_LENGTH.into()),
        ("TEXT_BYTES", TEXT_BYTES.into()),
        ("SECONDS_PER_DAY", SECONDS_PER_DAY.into()),
        ("DECODED_VALUE", DECODED_VALUE.into()),
        ("DECODED_ISSUES", DECODED_ISSUES.into()),
        ("DECODED_MALFORMED", DECODED_MALFORMED.into()),
        ("ANSWERED", ANSWERED.into()),
        ("INJECTION_UNBOUND", INJECTION_UNBOUND.into()),
        (
            "INJECTION_PROTOCOL_VIOLATION",
            INJECTION_PROTOCOL_VIOLATION.into(),
        ),
        ("HOST_EXCEPTION", HOST_EXCEPTION.into()),
        ("FAKE_NO_OUTPUT", FAKE_NO_OUTPUT.into()),
        ("HASH_START", HASH_START.into()),
        ("HASH_PRESENT", HASH_PRESENT.into()),
    ]
}

/// The contract as it is now, a line for each thing either side depends on.
fn surface() -> BTreeSet<String> {
    let mut lines = BTreeSet::new();
    for call in GENERATED_RUNTIME {
        lines.insert(format!(
            "generated {} {:?} -> {:?}",
            call.name, call.takes, call.answers
        ));
    }
    for function in HOST_RUNTIME {
        lines.insert(format!("host {function:?}"));
    }
    for case in HOST_CASES {
        lines.insert(format!("case {case:?}"));
    }
    for (name, status) in HOST_STATUSES.iter().chain(EXAMPLE_STATUSES) {
        lines.insert(format!("status {name} = {status}"));
    }
    lines.insert(format!("answers {IMPLEMENTATION_ANSWERS:?}"));
    lines.insert(format!("reserved {RESERVED:?}"));
    for word in HostWord::ALL {
        lines.insert(format!(
            "word {}: {:?}, {}",
            word.spelt(),
            word.representation(),
            if word.is_datum() { "datum" } else { "handle" }
        ));
    }
    for storage in HOST_STORAGE {
        lines.insert(format!(
            "storage {}: {} slots of {SLOT} bytes, aligned to {SLOT}",
            storage.name, storage.slots
        ));
    }
    for (name, promise) in HOST_INPUT_CONTRACT {
        lines.insert(format!("input {name}: {promise}"));
    }
    for (name, promise) in SCOPE_CONTRACT {
        lines.insert(format!("scope {name}: {promise}"));
    }
    for (at, rule) in HASHING.iter().enumerate() {
        lines.insert(format!("hashing {at} {rule}"));
    }
    lines.insert(format!("runtime says {}", runtime_generation_symbol()));
    for (name, value) in constants() {
        lines.insert(format!("const {name} = {value}"));
    }
    lines
}

fn record(generation: u32) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("generations")
        .join(format!("{generation}.txt"))
}

/// Writes the record for the generation this build is, for the day a generation moves:
/// `ABI_RECORD=1 cargo test -p souther-native-abi --test generation -- --ignored`. It refuses to
/// overwrite one, since a record is what its generation was.
#[test]
#[ignore = "writes a record"]
fn write_the_record_of_this_generation() {
    let path = record(ABI_GENERATION);
    assert!(!path.exists(), "{} is written already", path.display());
    let mut text = format!("generation {ABI_GENERATION}\n");
    for line in surface() {
        text.push_str(&line);
        text.push('\n');
    }
    std::fs::write(path, text).unwrap();
}

#[test]
fn the_current_generation_has_a_record_and_nothing_it_says_has_changed() {
    let path = record(ABI_GENERATION);
    let recorded = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "no record of generation {ABI_GENERATION} at {}: a generation that moves records the \
             contract it starts from (see `write_the_record_of_this_generation`)",
            path.display()
        )
    });
    let mut lines = recorded.lines();
    assert_eq!(
        lines.next(),
        Some(format!("generation {ABI_GENERATION}").as_str())
    );
    let now = surface();
    let moved: Vec<&str> = lines.filter(|line| !now.contains(*line)).collect();
    assert!(
        moved.is_empty(),
        "the contract of generation {ABI_GENERATION} moved, and an object built before it calls \
         what it no longer is. Add a generation to `GENERATIONS` and record it. What moved:\n{}",
        moved.join("\n")
    );
}

#[test]
fn every_generation_has_its_record() {
    for (generation, _) in GENERATIONS {
        // Generation 2 was the first to write one into a symbol, and nothing before generation 5 is
        // recorded: the record is kept from the generation that begins keeping it.
        if *generation >= FIRST_RECORDED {
            assert!(
                record(*generation).exists(),
                "no record of generation {generation}"
            );
        }
    }
}

/// A record is of a generation `GENERATIONS` names, and nothing else is kept beside the records: a
/// record of a generation the crate does not name is one nothing holds to the code, and whoever
/// reads the records by what is there, rather than by `ABI_GENERATION`, would take it for one.
#[test]
fn no_record_is_of_a_generation_the_crate_does_not_name() {
    let named: BTreeSet<u32> = GENERATIONS
        .iter()
        .map(|(generation, _)| *generation)
        .filter(|generation| *generation >= FIRST_RECORDED)
        .collect();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("generations");
    let mut stray = Vec::new();
    for entry in std::fs::read_dir(&directory).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let generation = name
            .strip_suffix(".txt")
            .and_then(|number| number.parse::<u32>().ok());
        if !generation.is_some_and(|it| named.contains(&it)) {
            stray.push(name);
        }
    }
    assert!(
        stray.is_empty(),
        "kept beside the records of the generations the crate names: {stray:?}"
    );
}

/// The first generation with a record, which is the generation this was written under.
const FIRST_RECORDED: u32 = 5;

/// A number the crate states is one the record holds. The list above is written by hand, and the
/// scan is what stops one added to the source and left out of it.
#[test]
fn every_number_the_crate_states_is_in_the_record() {
    let source =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .unwrap();
    let listed: BTreeSet<&str> = constants().iter().map(|(name, _)| *name).collect();
    let mut missing = Vec::new();
    let mut seen = 0;
    for line in source.lines() {
        let Some(rest) = line.strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, ty)) = rest.split_once(": ") else {
            continue;
        };
        let numeric = ["i64", "i32", "u32", "usize", "Status"]
            .iter()
            .any(|it| ty.starts_with(&format!("{it} =")));
        if !numeric || name == "ABI_GENERATION" {
            continue;
        }
        seen += 1;
        if !listed.contains(name) {
            missing.push(name);
        }
    }
    assert!(
        seen >= 20,
        "the scan found {seen} numbers: it reads nothing"
    );
    assert!(missing.is_empty(), "not in the record: {missing:?}");
}
