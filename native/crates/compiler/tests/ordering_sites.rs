//! Where two values are compared on an order: at a comparison written in source, and nowhere else.
//!
//! Comparing two values of an enumeration places each among its leaves, a walk of as many cases as
//! it has. A comparison written in source does that once, and a sort, a merge or a search for an
//! extreme compares each value many times, so they compare keys worked out once per value
//! (`SortKeys` in `lists.rs`): a sort that compared values would walk the leaves `n log n` times
//! where it needs to `n` times. That is a property of where `ordering::ordered` is called, which
//! nothing in the types stops a new walk from breaking, so it is held here: every call of it, by the
//! function it is written in.

use std::fs;
use std::path::Path;

/// Where `ordering::ordered` may be called from: the lowering of one comparison written in source,
/// and the one comparison of two keys that are values because their order is a primitive's.
const CALLED_FROM: &[(&str, &str)] = &[("lib.rs", "compare"), ("lists.rs", "ordered")];

#[test]
fn two_values_are_compared_on_an_order_only_where_one_comparison_is_written() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut files: Vec<_> = fs::read_dir(&source)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|it| it == "rs"))
        .collect();
    files.sort();
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let text = fs::read_to_string(&path).unwrap();
        let mut within = String::new();
        for line in text.lines() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            if let Some(at) = code.find("fn ")
                && (code.starts_with("fn ")
                    || code.starts_with("pub(crate) fn ")
                    || code.starts_with("pub fn ")
                    || code.starts_with("unsafe fn "))
            {
                let name = &code[at + 3..];
                within = name
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap()
                    .to_string();
                continue;
            }
            // A call of the function, spelt with its module or not; not a method of that name.
            let calls = code.contains("ordering::ordered(")
                || (code.contains("ordered(")
                    && !code.contains(".ordered(")
                    && !code.contains("_ordered(")
                    && file != "ordering.rs");
            if calls {
                found.push((file.clone(), within.clone()));
            }
        }
    }
    let expected: Vec<(String, String)> = CALLED_FROM
        .iter()
        .map(|(file, function)| (file.to_string(), function.to_string()))
        .collect();
    assert_eq!(
        found, expected,
        "`ordering::ordered` is called from somewhere that may compare one value many times; a \
         walk over many values compares `SortKeys`"
    );
}
