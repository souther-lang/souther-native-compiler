//! That the token at the front of a value is read in one place.
//!
//! A test of which case a value is loads through the value's address, and only a value whose type
//! says its case has a token there: over a number it is a read of memory the value does not own,
//! and nothing at run time says so. So the load is written once, in `Tagged::which`, and a `Tagged`
//! is made only by asking the type. A second load of `WHICH` written anywhere else would be a test
//! whose value nobody asked about, which is how one got past review once; this finds it by reading
//! the source rather than by anyone remembering to look.

use std::fs;
use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`.
fn sources(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("the crate's sources are readable") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            sources(&path, into);
        } else if path.extension().is_some_and(|it| it == "rs") {
            into.push(path);
        }
    }
}

/// Where every call to `.load(` in `source` starts, and its arguments up to the parenthesis that
/// closes it.
fn loads(source: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    for (at, _) in source.match_indices(".load(") {
        let open = at + ".load(".len();
        let mut depth = 1;
        let mut end = source.len();
        for (offset, character) in source[open..].char_indices() {
            match character {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                end = open + offset;
                break;
            }
        }
        found.push((at, &source[open..end]));
    }
    found
}

/// Where `Tagged::which` is in `source`, from its signature to the brace closing its body.
fn which_in(source: &str) -> Option<std::ops::Range<usize>> {
    let start = source.find("pub(crate) fn which(self")?;
    let end = start + source[start..].find("\n    }\n")?;
    Some(start..end)
}

#[test]
fn the_token_at_the_front_of_a_value_is_loaded_only_by_tagged() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(!files.is_empty());

    let mut reading = Vec::new();
    for file in &files {
        let source = fs::read_to_string(file).expect("a source file is readable");
        let which = which_in(&source);
        for (at, arguments) in loads(&source) {
            if arguments.contains("WHICH") {
                let line = source[..at].lines().count();
                let tagged = which.as_ref().is_some_and(|it| it.contains(&at));
                reading.push((file.display().to_string(), line, tagged));
            }
        }
    }
    assert_eq!(
        reading.len(),
        1,
        "the token is loaded at more than one place: {reading:?}"
    );
    assert!(
        reading[0].2,
        "the one load of the token is not `Tagged::which`: {reading:?}"
    );
}
