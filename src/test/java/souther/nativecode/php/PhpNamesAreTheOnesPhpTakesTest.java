package souther.nativecode.php;

import org.junit.jupiter.api.Test;
import souther.nativecode.Php;
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
        Set<String> candidates = new TreeSet<>(Php.ran(List.of(script.toString())).lines()
                .filter(it -> it.matches("[A-Za-z_][A-Za-z0-9_]*")).toList());
        candidates.addAll(PhpNames.reserved());
        candidates.addAll(PhpNames.unnameableParameters());
        // A reserved word with a letter past ASCII that a Unicode rule would make an ASCII one (the
        // Kelvin sign is a capital K to Java): not the reserved word to PHP.
        candidates.add("brea\u212A");

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

    /**
     * Two names PHP, or the file each class is written to, might take for one, held to whether a
     * binding's claims take them for one: a method's by what PHP compiles, a class's by that and by
     * whether this machine's file system keeps two files of those names apart.
     */
    @Test
    void twoNamesAreOneWhereWhatLooksThemUpSaysSo(@TempDir Path into) throws Exception {
        List<List<String>> pairs = List.of(
                List.of("Kept", "kept"),
                List.of("\u00c4", "\u00e4"),
                List.of("K", "\u212A"),
                List.of("Stra\u00dfe", "STRASSE"),
                List.of("Kept", "Kepts"));
        for (List<String> pair : pairs) {
            String one = pair.get(0);
            String other = pair.get(1);

            Path methods = Files.createTempFile(into, "methods", ".php");
            Files.writeString(methods, "<?php class C { public function " + one
                    + "() {} public function " + other + "() {} }\n", StandardCharsets.UTF_8);
            boolean phpMethods = !Php.compiles(methods);
            assertThat(claimsOne(PhpNames.Claimed.methods("a class"), one, other))
                    .as("the methods %s and %s", one, other).isEqualTo(phpMethods);

            Path classes = Files.createTempFile(into, "classes", ".php");
            Files.writeString(classes, "<?php class " + one + " {} class " + other + " {}\n",
                    StandardCharsets.UTF_8);
            Path directory = Files.createTempDirectory(into, "files");
            Files.writeString(directory.resolve(one + ".php"), "one", StandardCharsets.UTF_8);
            Files.writeString(directory.resolve(other + ".php"), "other", StandardCharsets.UTF_8);
            boolean oneFile;
            try (var files = Files.list(directory)) {
                oneFile = files.count() == 1;
            }
            boolean phpClasses = !Php.compiles(classes);
            if (phpClasses || oneFile) {
                assertThat(claimsOne(PhpNames.Claimed.classes("a namespace"), one, other))
                        .as("the classes %s and %s", one, other).isTrue();
            }
        }
    }

    private static boolean claimsOne(PhpNames.Claimed claimed, String one, String other) {
        claimed.claim(one, "one");
        try {
            claimed.claim(other, "other");
            return false;
        } catch (PhpBindings.NotBindable refused) {
            return true;
        }
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
            return !Php.compiles(file);
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    private static String phpVersion() {
        try {
            return Php.ran(List.of("-r", "echo PHP_VERSION;"));
        } catch (Exception e) {
            return "?";
        }
    }
}
