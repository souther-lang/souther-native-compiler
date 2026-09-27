//! An object and a runtime of different generations do not link.
//!
//! The functions generated code calls in the runtime are the runtime's own and carry no
//! generation, so a linker resolves `souther_string_concat` to whatever defines it, however
//! differently the call to it is made. What the generation is held to on that side is a symbol only
//! the runtime of that generation defines, which every object refers to: an object of one
//! generation linked with the runtime of another has an undefined symbol, and never a call made one
//! way and answered the other (souther-native-compiler#107).

use souther_native_abi::runtime_generation_symbol;
use souther_native_driver::object_for;
use std::path::Path;
use std::process::Command;

mod support;
use support::PREFIX;

/// A document small enough to say nothing but what every object says.
const DOCUMENT: &str = include_str!("adding.transport.json");

/// The names of the symbols of the runtime's kind that `object` refers to and does not define, as
/// the linker spells them: `nm -u` says one to a line, its name last, and says it the same way on
/// every system this runs on, where its other flags do not. Only those the runtime defines are
/// asked of it: a linker supplies the rest.
fn undefined(object: &Path) -> Vec<String> {
    let said = Command::new("nm")
        .arg("-u")
        .arg(object)
        .output()
        .expect("nm to run");
    assert!(
        said.status.success(),
        "{}",
        String::from_utf8_lossy(&said.stderr)
    );
    String::from_utf8(said.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .filter_map(|name| name.strip_prefix(PREFIX))
        .filter(|name| name.starts_with("souther"))
        .map(str::to_string)
        .collect()
}

#[test]
fn every_object_refers_to_the_symbol_of_the_runtime_it_is_of() {
    let dir = tempfile::tempdir().unwrap();
    let object = dir.path().join("a.o");
    std::fs::write(&object, object_for(DOCUMENT).expect("an object")).unwrap();
    assert!(
        undefined(&object).contains(&runtime_generation_symbol()),
        "an object that refers to no symbol of its runtime's generation links with any runtime"
    );
}

/// The runtime the tests build is of the generation the object is: they link, which they do only
/// where the runtime defines the symbol the object refers to. Asked of the linker and not of the
/// archive, which a system's `nm` cannot always read.
#[test]
fn the_runtime_defines_the_symbol_of_its_generation() {
    let dir = tempfile::tempdir().unwrap();
    let object = dir.path().join("a.o");
    std::fs::write(&object, object_for(DOCUMENT).expect("an object")).unwrap();
    let main = dir.path().join("main.c");
    std::fs::write(&main, "int main(void) { return 0; }\n").unwrap();
    let mut command = Command::new("cc");
    command
        .arg(&object)
        .arg(&main)
        .arg("-o")
        .arg(dir.path().join("linked"));
    command.args(
        souther_native_driver::runtime_arguments(support::runtime())
            .expect("the requirements written beside the archive"),
    );
    let linked = command.output().expect("cc to run");
    assert!(
        linked.status.success(),
        "an object of this generation does not link with the runtime of it: {}",
        String::from_utf8_lossy(&linked.stderr)
    );
}

/// The linker is what refuses, and this asks it: a runtime that defines everything an object refers
/// to but the symbol of the generation is one that does not link, and one that defines that too
/// does. The stand-in is made from the object's own undefined symbols, so it is a runtime of every
/// call and of no generation.
#[test]
fn a_runtime_of_another_generation_is_an_undefined_symbol() {
    let dir = tempfile::tempdir().unwrap();
    let object = dir.path().join("a.o");
    std::fs::write(&object, object_for(DOCUMENT).expect("an object")).unwrap();
    let sentinel = runtime_generation_symbol();
    let stand_in = |name: &str, defining: &[String]| {
        let mut assembly = String::from("  .text\n");
        for symbol in defining {
            assembly.push_str(&format!(
                "  .globl \"{PREFIX}{symbol}\"\n\"{PREFIX}{symbol}\":\n  ret\n"
            ));
        }
        let source = dir.path().join(format!("{name}.s"));
        std::fs::write(&source, assembly).unwrap();
        let built = dir.path().join(format!("{name}.o"));
        let ran = Command::new("cc")
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&built)
            .output()
            .expect("cc to run");
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
        built
    };
    let mut wanted = undefined(&object);
    wanted.push("main".to_string());
    let without: Vec<String> = wanted
        .iter()
        .filter(|it| **it != sentinel)
        .cloned()
        .collect();
    let link = |runtime: &Path| {
        Command::new("cc")
            .arg(&object)
            .arg(runtime)
            .arg("-o")
            .arg(dir.path().join("linked"))
            .output()
            .expect("cc to run")
    };

    let refused = link(&stand_in("other", &without));
    let said = String::from_utf8_lossy(&refused.stderr);
    assert!(
        !refused.status.success(),
        "linked with a runtime of no generation"
    );
    assert!(said.contains(&sentinel), "{said}");

    let mut with = without;
    with.push(sentinel);
    let linked = link(&stand_in("this", &with));
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
}
