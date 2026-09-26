package souther.bindings;

import org.junit.jupiter.api.Test;

import java.lang.reflect.Constructor;
import java.lang.reflect.Modifier;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What only one factory makes, so that what the factory holds it to cannot be gone round. A
 * public constructor, or a subclass, would make one the factory never saw.
 */
class WhatOnlyAFactoryMakesTest {

    /**
     * A manifest is made by reading one and no other way: its own constructor holds what
     * constructing a behavior requires closed over every module, which no part of it can hold
     * alone.
     */
    @Test
    void aManifestIsMadeOnlyByReadingOne() {
        assertMadeOnlyByItsOwn(Manifest.class);
    }

    /**
     * An output is made only where the directory it replaces was found to be a binding of the
     * same host, and what it writes is marked as one: made any other way, {@code commit} would put
     * a directory nothing marked in place of one nothing checked.
     */
    @Test
    void anOutputIsMadeOnlyWhereWhatItReplacesWasChecked() {
        assertMadeOnlyByItsOwn(Output.class);
    }

    private static void assertMadeOnlyByItsOwn(Class<?> type) {
        assertThat(type.getDeclaredConstructors()).as("constructors of %s", type)
                .extracting(Constructor::getModifiers)
                .allMatch(Modifier::isPrivate);
        assertThat(Modifier.isFinal(type.getModifiers())).as("%s is final", type).isTrue();
    }
}
