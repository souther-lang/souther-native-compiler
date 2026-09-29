package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import souther.bindings.PublicReach;

/**
 * What the Rust generator offers to the command is public all the way out: a type in a signature the
 * command calls is one it can name, and what nothing public reaches stays as private as it is
 * written.
 */
class WhatARustGeneratorReachesIsPublicTest {

    @Test
    void everyTypeAPublicSignatureReachesIsPublic() throws Exception {
        PublicReach.everyTypeAPublicSignatureReachesIsPublic(RustBindings.class, RustBindings.class,
                RustBindingGenerator.class);
    }
}
