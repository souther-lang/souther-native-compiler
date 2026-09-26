package souther.bindings;

import org.junit.jupiter.api.Test;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.GenericArrayType;
import java.lang.reflect.Member;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.lang.reflect.TypeVariable;
import java.lang.reflect.WildcardType;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What the generators' packages offer is used from other packages: a host's generator takes a
 * manifest apart in its own, and the command line calls a generator from the compiler's. So every
 * type a public signature under {@code souther.bindings} reaches, as what it answers or takes, a
 * record's component, a type argument, a sealed type's cases or a public type nested in one, is
 * public all the way out. What nothing public reaches, such as a reader's own helpers, stays as
 * private as it is written.
 *
 * <p>Walked from every public type in those packages as the build compiled them, so a type added
 * later is held to this as soon as it is added, and not once a generator in another package reaches
 * for it.
 *
 * <p>In the package every generator shares, a member of a type it reaches is public or private: a
 * package-private one is one nothing outside the package can call, and a generator is outside it.
 */
class WhatAGeneratorReachesIsPublicTest {

    private static final String ROOT = "souther.bindings";

    @Test
    void everyTypeAPublicSignatureReachesIsPublic() throws Exception {
        Walk walk = new Walk();
        for (Class<?> type : publicTopLevelTypes()) {
            walk.visit(type, "the package");
        }
        assertThat(walk.visited).contains(Manifest.class, Manifest.Module.class,
                Manifest.Shape.class, Manifest.Reach.class);
        assertThat(walk.refused).isEmpty();
    }

    @Test
    void aMemberOfWhatTheSharedPackageOffersIsPublicOrPrivate() throws Exception {
        Walk walk = new Walk();
        for (Class<?> type : publicTopLevelTypes()) {
            walk.visit(type, "the package");
        }
        List<String> neither = new ArrayList<>();
        for (Class<?> type : walk.visited) {
            if (!type.getPackageName().equals(ROOT)) {
                continue;
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

    /** Every public type declared at the top of a file in the generators' packages. */
    private static List<Class<?>> publicTopLevelTypes() throws Exception {
        Path classes = Path.of(Manifest.class.getProtectionDomain().getCodeSource().getLocation()
                .toURI());
        Path under = classes.resolve(ROOT.replace('.', '/'));
        List<Class<?>> types = new ArrayList<>();
        try (Stream<Path> files = Files.walk(under)) {
            for (Path file : files.filter(it -> it.toString().endsWith(".class")
                    && !it.getFileName().toString().contains("$")).sorted().toList()) {
                String name = classes.relativize(file).toString()
                        .replace(file.getFileSystem().getSeparator(), ".")
                        .replaceAll("\\.class$", "");
                Class<?> type = Class.forName(name);
                if (Modifier.isPublic(type.getModifiers())) {
                    types.add(type);
                }
            }
        }
        assertThat(types).as("public types under %s", under).contains(Manifest.class);
        return types;
    }

    /** What a walk from public signatures has reached, and what it reached that is not public. */
    private static final class Walk {

        final Set<Class<?>> visited = new HashSet<>();
        final List<String> refused = new ArrayList<>();

        void visit(Class<?> reached, String via) {
            Class<?> type = reached.isArray() ? elementOf(reached) : reached;
            if (!type.getPackageName().startsWith(ROOT) || !visited.add(type)) {
                return;
            }
            if (!publicAllTheWayOut(type)) {
                refused.add(type.getName() + ", reached through " + via);
                return;
            }
            for (Method method : type.getDeclaredMethods()) {
                if (Modifier.isPublic(method.getModifiers()) && !method.isSynthetic()) {
                    reach(method.getGenericReturnType(), method);
                    Stream.of(method.getGenericParameterTypes()).forEach(it -> reach(it, method));
                    Stream.of(method.getGenericExceptionTypes()).forEach(it -> reach(it, method));
                }
            }
            for (Constructor<?> constructor : type.getDeclaredConstructors()) {
                if (Modifier.isPublic(constructor.getModifiers())) {
                    Stream.of(constructor.getGenericParameterTypes())
                            .forEach(it -> reach(it, constructor));
                }
            }
            for (Field field : type.getDeclaredFields()) {
                if (Modifier.isPublic(field.getModifiers())) {
                    reach(field.getGenericType(), field);
                }
            }
            if (type.isRecord()) {
                for (RecordComponent component : type.getRecordComponents()) {
                    reach(component.getGenericType(), type.getName() + "." + component.getName());
                }
            }
            if (type.getGenericSuperclass() != null) {
                reach(type.getGenericSuperclass(), type.getName());
            }
            Stream.of(type.getGenericInterfaces()).forEach(it -> reach(it, type.getName()));
            if (type.isSealed()) {
                Stream.of(type.getPermittedSubclasses())
                        .forEach(it -> visit(it, "the cases of " + type.getName()));
            }
            for (Class<?> nested : type.getDeclaredClasses()) {
                if (Modifier.isPublic(nested.getModifiers())) {
                    visit(nested, type.getName());
                }
            }
        }

        private void reach(Type type, Object via) {
            switch (type) {
                case Class<?> it -> visit(it, via.toString());
                case ParameterizedType it -> {
                    reach(it.getRawType(), via);
                    Stream.of(it.getActualTypeArguments()).forEach(arg -> reach(arg, via));
                }
                case WildcardType it -> {
                    Stream.of(it.getUpperBounds()).forEach(bound -> reach(bound, via));
                    Stream.of(it.getLowerBounds()).forEach(bound -> reach(bound, via));
                }
                case GenericArrayType it -> reach(it.getGenericComponentType(), via);
                case TypeVariable<?> it -> Stream.of(it.getBounds()).forEach(b -> reach(b, via));
                default -> throw new IllegalStateException("a type reflection does not name: "
                        + type);
            }
        }

        private static Class<?> elementOf(Class<?> array) {
            Class<?> element = array;
            while (element.isArray()) {
                element = element.getComponentType();
            }
            return element;
        }

        private static boolean publicAllTheWayOut(Class<?> type) {
            for (Class<?> at = type; at != null; at = at.getEnclosingClass()) {
                if (!Modifier.isPublic(at.getModifiers())) {
                    return false;
                }
            }
            return true;
        }
    }
}
