package souther.bindings.php;

import org.junit.jupiter.api.Test;
import souther.bindings.PublicReach;

/**
 * What the Php generator offers to the command is public all the way out: a type in a signature the
 * command calls is one it can name, and what nothing public reaches stays as private as it is
 * written.
 */
class WhatAPhpGeneratorReachesIsPublicTest {

    @Test
    void everyTypeAPublicSignatureReachesIsPublic() throws Exception {
        PublicReach.everyTypeAPublicSignatureReachesIsPublic(PhpBindings.class, PhpBindings.class,
                PhpBindingGenerator.class);
    }
}
