package souther.bindings;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.EnabledIfSystemProperty;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a generator compiled against {@link BindingApi#MAJOR} depends on is still true of the classes.
 *
 * <p>The major is a number somebody moves by hand, on the day the API stops being what a generator
 * compiled before expects; nothing fails on that day if the number is left where it is. So each major
 * has a record of its surface as it was when it began ({@code generations/<major>.txt}, written by
 * {@link #writeTheRecordOfThisMajor}), and what is held is that nothing the record of the current
 * major says has changed. A line added is a type or a member no generator of that major uses, and is
 * compatible; a line changed or gone is a surface that moved, and needs the next major, which records
 * the surface it begins with.
 */
class WhatAMajorPromisesStillHoldsTest {

    @Test
    void nothingTheRecordOfTheCurrentMajorSaysHasChanged() throws Exception {
        Path record = record(BindingApi.MAJOR);
        assertThat(record).as("the record of major %d: a major that moves records the surface it"
                + " begins with (see writeTheRecordOfThisMajor)", BindingApi.MAJOR).exists();
        List<String> recorded = Files.readAllLines(record, StandardCharsets.UTF_8);
        assertThat(recorded.getFirst()).isEqualTo("major " + BindingApi.MAJOR);

        Set<String> now = ApiSurface.of(BindingApi.class);
        List<String> moved = new ArrayList<>();
        for (String line : recorded.subList(1, recorded.size())) {
            if (!now.contains(line)) {
                moved.add(line);
            }
        }
        assertThat(moved).as("the surface of major %d moved, and a generator compiled against it"
                + " no longer means what it did: raise BindingApi.MAJOR and record it. What moved",
                BindingApi.MAJOR).isEmpty();
    }

    @Test
    void theSurfaceHoldsWhatAGeneratorSwitchesOverAndImplementsWhole() throws Exception {
        Set<String> now = ApiSurface.of(BindingApi.class);

        assertThat(now).anyMatch(it -> it.startsWith("sealed ValueCrossing = "));
        assertThat(now).anyMatch(it -> it.startsWith("enum Manifest.Primitive = "));
        assertThat(now).anyMatch(it -> it.startsWith("record BindingInput = (Manifest manifest,"));
        assertThat(now).contains("abstract BindingGenerator = generate(BindingInput, java.nio.file.Path,"
                        + " java.util.Map<java.lang.String, java.lang.String>) -> void throws"
                        + " java.io.IOException,preflight(java.util.Map<java.lang.String,"
                        + " java.lang.String>) -> void");
        assertThat(now).anyMatch(it -> it.startsWith("abstract Declarations = copyTo("));
        assertThat(now).as("declared nullness is part of a line")
                .anyMatch(it -> it.contains("decodeHost() -> @Nullable Manifest.Function"));
    }

    /** Only the records of majors up to this one, each of which names its own major. */
    @Test
    void everyRecordIsOfAMajorThatWas() throws IOException {
        try (Stream<Path> records = Files.list(record(BindingApi.MAJOR).getParent())) {
            for (Path each : records.toList()) {
                String file = each.getFileName().toString();
                assertThat(file).matches("[1-9][0-9]*\\.txt");
                int major = Integer.parseInt(file.substring(0, file.length() - 4));
                assertThat(major).isLessThanOrEqualTo(BindingApi.MAJOR);
                assertThat(Files.readAllLines(each).getFirst()).isEqualTo("major " + major);
            }
        }
    }

    /**
     * Writes the record of this major, for the day the major moves:
     * {@code mvn -pl bindings/api test -Dtest=WhatAMajorPromisesStillHoldsTest
     * -Dsouther.bindings.record=true}. It refuses to overwrite one, since a record is what its major
     * was.
     */
    @Test
    @EnabledIfSystemProperty(named = "souther.bindings.record", matches = "true")
    void writeTheRecordOfThisMajor() throws Exception {
        Path record = record(BindingApi.MAJOR);
        assertThat(record).as("written already").doesNotExist();
        StringBuilder text = new StringBuilder("major " + BindingApi.MAJOR + "\n");
        for (String line : ApiSurface.of(BindingApi.class)) {
            text.append(line).append('\n');
        }
        Files.createDirectories(record.getParent());
        Files.writeString(record, text, StandardCharsets.UTF_8);
    }

    private static Path record(int major) {
        return Path.of(System.getProperty("souther.repository"), "bindings", "api", "generations",
                major + ".txt");
    }
}
