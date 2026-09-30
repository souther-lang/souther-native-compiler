package souther.bindings;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.EnabledIfSystemProperty;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a generator compiled against {@link BindingApi#MAJOR} depends on is still true of the classes.
 *
 * <p>A generator compiled against major N may use anything the API offered under N on the day it was
 * compiled, and that day can be any day of N's life. So the record of a major is a ledger of every
 * line the API has ever offered under it ({@code generations/<major>.txt}), and it only grows: a line
 * the classes no longer have is a promise broken, and needs the next major, which records the surface
 * it begins with; a line the classes have and the record does not is an addition, compatible, and
 * recorded under the current major before it is released, so that removing it later is seen as the
 * break it is. Both fail here, each saying which it is. {@link #recordThisMajor} writes the record,
 * and refuses to where a recorded line is gone.
 */
class WhatAMajorPromisesStillHoldsTest {

    @Test
    void theCurrentMajorsLedgerIsTheSurfaceExactly() throws Exception {
        Path record = record(BindingApi.MAJOR);
        assertThat(record).as("the record of major %d: a major that moves records the surface it"
                + " begins with (see recordThisMajor)", BindingApi.MAJOR).exists();
        List<String> recorded = recorded(record);
        Set<String> now = ApiSurface.of(BindingApi.class);

        assertThat(gone(recorded, now)).as("the surface of major %d lost what it promised, and a"
                + " generator compiled against it no longer runs as it did: raise BindingApi.MAJOR and"
                + " record the new major. What is gone", BindingApi.MAJOR).isEmpty();
        assertThat(added(recorded, now)).as("the surface of major %d gained what it has not"
                + " recorded; it is compatible, and is recorded under major %d so that taking it away"
                + " later is seen (see recordThisMajor). What was added", BindingApi.MAJOR,
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
     * Records the surface under this major:
     * {@code mvn -pl bindings/api test -Dtest=WhatAMajorPromisesStillHoldsTest
     * -Dsouther.bindings.record=true}. A major's first record is its whole surface; after that the
     * record only grows, and a surface that lost a recorded line is refused here, since that needs the
     * next major and not a record of this one.
     */
    @Test
    @EnabledIfSystemProperty(named = "souther.bindings.record", matches = "true")
    void recordThisMajor() throws Exception {
        Path record = record(BindingApi.MAJOR);
        Set<String> now = ApiSurface.of(BindingApi.class);
        if (Files.exists(record)) {
            assertThat(gone(recorded(record), now)).as("not recorded: what major %d promised is"
                    + " gone, which needs the next major", BindingApi.MAJOR).isEmpty();
        }
        StringBuilder text = new StringBuilder("major " + BindingApi.MAJOR + "\n");
        for (String line : now) {
            text.append(line).append('\n');
        }
        Files.createDirectories(record.getParent());
        Files.writeString(record, text, StandardCharsets.UTF_8);
    }

    private static List<String> recorded(Path record) throws IOException {
        List<String> lines = Files.readAllLines(record, StandardCharsets.UTF_8);
        assertThat(lines.getFirst()).isEqualTo("major " + BindingApi.MAJOR);
        return lines.subList(1, lines.size());
    }

    private static List<String> gone(List<String> recorded, Set<String> now) {
        return recorded.stream().filter(line -> !now.contains(line)).toList();
    }

    private static List<String> added(List<String> recorded, Set<String> now) {
        Set<String> held = new java.util.HashSet<>(recorded);
        return now.stream().filter(line -> !held.contains(line)).toList();
    }

    private static Path record(int major) {
        return Path.of(System.getProperty("souther.repository"), "bindings", "api", "generations",
                major + ".txt");
    }
}
