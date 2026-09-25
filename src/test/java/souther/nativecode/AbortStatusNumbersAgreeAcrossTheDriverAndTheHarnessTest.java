package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * {@link Running#abortKindOf}'s table, held to the fixture {@code
 * native/crates/compiler/tests/abort_status.rs} writes {@code native_status} against.
 *
 * <p>Two hand-written copies of one mapping and neither holds the other to anything: {@code
 * native_status} is the driver's own answer, {@code abortKindOf} is this test harness's copy of
 * it, made because a run's own status has to be turned back into the {@link AbortKind} it came
 * from to assert anything about which one it was, on the far side of a process boundary neither
 * copy can read through. A status numbered differently on the two sides reads without complaint
 * and answers the wrong reason for it — the same failure mode {@code
 * WhatBothHalvesSpellTheSameWayTest} closes for {@code Op} and {@code Prim}, closed here the same
 * way: the fixture is what one side wrote, and this asserts the other reads every member of it
 * back as the member it names.
 */
class AbortStatusNumbersAgreeAcrossTheDriverAndTheHarnessTest {

    /**
     * The document {@code native/crates/compiler/tests/abort_status.rs} is held to. Read here
     * rather than described again, for the reason every other fixture this project checks in is.
     */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "abort-status-abi4.json");

    @Test
    void everyStatusTheDriverAnswersIsReadBackAsTheAbortKindItNames() throws IOException {
        Map<String, Integer> written = parsed(Files.readString(FIXTURE, StandardCharsets.UTF_8));

        assertThat(written).as("abort-status-abi4.json").hasSize(AbortKind.values().length);
        for (Map.Entry<String, Integer> entry : written.entrySet()) {
            AbortKind expected = AbortKind.valueOf(entry.getKey());
            assertThat(Running.abortKindOf(entry.getValue()))
                    .as("status %d, which the fixture names %s", entry.getValue(), entry.getKey())
                    .isEqualTo(expected);
        }
    }

    /**
     * {@code {"A":1,"B":2}} read as a name and the number after it, in whatever order the
     * document lists them — the one shape this fixture is and no more general a reader than that.
     */
    private static Map<String, Integer> parsed(String json) {
        String inner = json.strip();
        inner = inner.substring(1, inner.length() - 1);
        Map<String, Integer> found = new LinkedHashMap<>();
        for (String entry : inner.split(",")) {
            String[] pair = entry.split(":");
            found.put(pair[0].strip().replace("\"", ""), Integer.parseInt(pair[1].strip()));
        }
        return found;
    }
}
