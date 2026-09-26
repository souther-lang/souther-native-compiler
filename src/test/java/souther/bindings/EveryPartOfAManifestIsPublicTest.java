package souther.bindings;

import org.junit.jupiter.api.Test;

import java.lang.reflect.Constructor;
import java.lang.reflect.Member;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a manifest says is read in this package and taken apart in every host's: each type nested in
 * {@link Manifest} and each member of one is public, or private and nobody's but its own. A member
 * left package-private is one a generator in another package finds missing only once it reaches
 * for it, and a type added later is held to this as soon as it is added.
 */
class EveryPartOfAManifestIsPublicTest {

    @Test
    void everyTypeAndMemberIsPublicOrPrivate() {
        List<String> neither = new ArrayList<>();
        for (Class<?> type : typesOf(Manifest.class)) {
            if (!Modifier.isPublic(type.getModifiers()) && !Modifier.isPrivate(type.getModifiers())) {
                neither.add(type.getName());
            }
            Stream.of(type.getDeclaredMethods(), type.getDeclaredFields(),
                            type.getDeclaredConstructors())
                    .flatMap(Stream::of)
                    .map(Member.class::cast)
                    .filter(member -> !member.isSynthetic())
                    .filter(member -> !Modifier.isPublic(member.getModifiers())
                            && !Modifier.isPrivate(member.getModifiers()))
                    .forEach(member -> neither.add(member.toString()));
        }
        assertThat(neither).isEmpty();
    }

    /** A manifest is made by reading one and no other way, so what a generator holds was read. */
    @Test
    void aManifestIsMadeOnlyByReadingOne() {
        assertThat(Manifest.class.getDeclaredConstructors())
                .extracting(Constructor::getModifiers)
                .allMatch(Modifier::isPrivate);
        assertThat(Modifier.isFinal(Manifest.class.getModifiers())).isTrue();
    }

    private static List<Class<?>> typesOf(Class<?> outer) {
        List<Class<?>> types = new ArrayList<>();
        types.add(outer);
        for (Class<?> nested : outer.getDeclaredClasses()) {
            types.addAll(typesOf(nested));
        }
        return types;
    }
}
