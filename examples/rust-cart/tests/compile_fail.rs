//! What the types refuse. Each file under `ui/` is a mistake a host of the model could make, and
//! its `.stderr` is what rustc says about it; none of them reaches a run.

#[test]
fn what_the_types_refuse() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
