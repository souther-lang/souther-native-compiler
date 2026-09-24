package souther.nativecode.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.function.BiConsumer;
import java.util.stream.Collectors;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What {@link PhpNames} refuses, held to what PHP refuses, asked of the PHP the tests run.
 *
 * <p>The rules are written from what PHP says, and a list written from memory is one a name PHP
 * refuses can be missing from ({@code $GLOBALS} as a parameter was). So PHP is asked: every word
 * its tokenizer names a token for, every superglobal, and every word the lists here name, each put
 * where a binding puts a name (a class, a method, a parameter) and compiled. A name PHP refuses
 * there and {@link PhpNames} takes would be a binding that does not load.
 */
class PhpNamesAreTheOnesPhpTakesTest {

    /** The words to ask about, as PHP names them. */
    private static final String CANDIDATES = """
            <?php
            $words = [];
            for ($id = 0; $id < 1024; $id++) {
                $name = token_name($id);
                if ($name !== 'UNKNOWN' && str_starts_with($name, 'T_')) {
                    $words[] = strtolower(substr($name, 2));
                }
            }
            foreach (['GLOBALS', '_SERVER', '_GET', '_POST', '_FILES', '_COOKIE', '_SESSION',
                      '_REQUEST', '_ENV', 'this'] as $superglobal) {
                $words[] = $superglobal;
            }
            echo implode("\\n", array_unique($words)), "\\n";
            """;

    /** Where a binding puts a name, as PHP it compiles. */
    private enum Place {
        CLASS("<?php class %s {}\n"),
        METHOD("<?php class C { public function %s() {} }\n"),
        PARAMETER("<?php function f($%s) {}\n");

        final String written;

        Place(String written) {
            this.written = written;
        }
    }

    @Test
    void everyNamePhpRefusesIsRefused(@TempDir Path into) throws Exception {
        Path script = into.resolve("candidates.php");
        Files.writeString(script, CANDIDATES, StandardCharsets.UTF_8);
        Set<String> candidates = new TreeSet<>(said(List.of("php", script.toString())).lines()
                .filter(it -> it.matches("[A-Za-z_][A-Za-z0-9_]*")).toList());
        candidates.addAll(PhpNames.reserved());
        candidates.addAll(PhpNames.unnameableParameters());

        Set<String> missed = new TreeSet<>();
        Set<String> needless = new TreeSet<>();
        candidates.parallelStream().forEach(word -> {
            for (Place place : Place.values()) {
                boolean phpRefuses = refusedByPhp(into, place, word);
                boolean refused = refusedHere(place, word);
                BiConsumer<Set<String>, String> note = (set, it) -> {
                    synchronized (set) {
                        set.add(it);
                    }
                };
                if (phpRefuses && !refused) {
                    note.accept(missed, place + " " + word);
                } else if (!phpRefuses && refused) {
                    note.accept(needless, place + " " + word);
                }
            }
        });

        assertThat(missed).as("names PHP refuses and a binding would write").isEmpty();
        // Refusing more than PHP does costs a model a name it could have had, and is kept only for
        // what PHP's manual reserves for later, which PHP takes today.
        assertThat(needless).as("names refused that PHP %s takes", phpVersion())
                .isSubsetOf(PhpNames.softReserved().stream().map(it -> Place.CLASS + " " + it)
                        .collect(Collectors.toSet()));
    }

    private static boolean refusedHere(Place place, String word) {
        try {
            switch (place) {
                case CLASS -> PhpNames.typeName(word, "a class");
                case METHOD -> PhpNames.memberName(word, "a method");
                case PARAMETER -> PhpNames.parameterName(word, "a parameter");
            }
            return false;
        } catch (PhpBindings.NotBindable refused) {
            return true;
        }
    }

    private static boolean refusedByPhp(Path into, Place place, String word) {
        try {
            Path file = Files.createTempFile(into, place.name(), ".php");
            Files.writeString(file, place.written.formatted(word), StandardCharsets.UTF_8);
            Process lint = new ProcessBuilder("php", "-l", file.toString())
                    .redirectErrorStream(true).start();
            lint.getInputStream().readAllBytes();
            return lint.waitFor() != 0;
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    private static String phpVersion() {
        try {
            return said(List.of("php", "-r", "echo PHP_VERSION;"));
        } catch (Exception e) {
            return "?";
        }
    }

    private static String said(List<String> command) throws Exception {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError(command.get(0) + " failed: " + said);
        }
        return said;
    }
}
