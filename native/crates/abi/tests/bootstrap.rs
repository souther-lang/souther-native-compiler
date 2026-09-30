//! The query a host asks a library's generation with is the same for every generation.
//!
//! Every other fact of the contract is held by the record of the generation it belongs to, and a
//! generation that moves may change it. This one may never change: it is how a host finds out
//! which generation it has loaded, so a host written for any generation has to be able to ask it of
//! a library of any other. So it is held here to what it was when it was made, and this test is
//! never updated to follow a change: a change is a mistake, whatever else moves with it.

use souther_native_abi::{GENERATION_QUERY, GENERATION_QUERY_DECLARED};

#[test]
fn the_generation_query_is_what_it_was_made_as() {
    assert_eq!(GENERATION_QUERY, "souther_abi_generation");
    assert_eq!(
        GENERATION_QUERY_DECLARED,
        "uint32_t souther_abi_generation(void);"
    );
}
