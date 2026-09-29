package souther.bindings;

import org.junit.jupiter.api.Test;

/** What the shared package offers to the generators is public all the way out. */
class WhatAGeneratorReachesIsPublicTest {

    @Test
    void everyTypeAPublicSignatureReachesIsPublic() throws Exception {
        PublicReach.everyTypeAPublicSignatureReachesIsPublic(Manifest.class, Manifest.class,
                Manifest.Module.class, Manifest.Shape.class, Manifest.Reach.class);
    }

    @Test
    void aMemberOfWhatTheSharedPackageOffersIsPublicOrPrivate() throws Exception {
        PublicReach.aMemberOfWhatTheSharedPackageOffersIsPublicOrPrivate(Manifest.class);
    }
}
