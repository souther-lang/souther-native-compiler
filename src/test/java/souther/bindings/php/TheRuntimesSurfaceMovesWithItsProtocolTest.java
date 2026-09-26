package souther.bindings.php;

import org.junit.jupiter.api.Test;
import souther.nativecode.Php;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What the runtime package offers a generated binding is what the protocol it speaks says it is.
 *
 * <p>A binding refuses to load over a runtime of another protocol, and that refusal is worth only
 * what the number is: a runtime that gains {@code Session::decimal} and keeps the number lets a
 * binding written against it load over one without, and the call fails later, in the middle of a
 * run. So the runtime's public surface — every class, and every public constant, property and
 * method, as PHP reflects them — is recorded under the protocol that has it, and the surface this
 * build has is held to the record for {@code Binding::PROTOCOL}. A change to it fails here until
 * the protocol moves (a line in {@code Binding::MOVES}) and the new surface is recorded under the
 * new number; the records of versions before it stay as they were.
 */
class TheRuntimesSurfaceMovesWithItsProtocolTest {

    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    /** Where each protocol's surface is recorded, one file for each, named by the number. */
    private static final Path RECORDED =
            Path.of("src", "test", "resources", "souther", "bindings", "php-runtime-surface");

    /**
     * Every class of the runtime package, and what of each is public, one line for each, sorted.
     * Modifiers are spelt here rather than asked of PHP as names, which PHP spells differently
     * across the versions the package runs on.
     */
    private static final String SURFACE = """
            require $argv[1] . '/vendor/autoload.php';
            $lines = [];
            $type = static fn (?ReflectionType $t): string => $t === null ? '-' : (string) $t;
            $flags = static fn (array $held): string => implode(' ', array_keys(array_filter($held)));
            foreach (glob($argv[1] . '/src/*.php') as $file) {
                $name = 'Souther\\\\Runtime\\\\' . basename($file, '.php');
                $class = new ReflectionClass($name);
                $lines[] = "$name " . $flags(['interface' => $class->isInterface(),
                        'abstract' => $class->isAbstract() && !$class->isInterface(),
                        'final' => $class->isFinal(), 'readonly' => $class->isReadOnly(),
                        'enum' => $class->isEnum()])
                    . ' extends ' . ($class->getParentClass() ? $class->getParentClass()->getName() : '-')
                    . ' implements ' . implode(',', $class->getInterfaceNames());
                foreach ($class->getReflectionConstants(ReflectionClassConstant::IS_PUBLIC) as $it) {
                    if ($it->getDeclaringClass()->getName() === $name) {
                        $lines[] = "$name::{$it->getName()} constant";
                    }
                }
                foreach ($class->getProperties(ReflectionProperty::IS_PUBLIC) as $it) {
                    if ($it->getDeclaringClass()->getName() === $name) {
                        $lines[] = "$name::\\${$it->getName()} " . $type($it->getType()) . ' '
                            . $flags(['static' => $it->isStatic(), 'readonly' => $it->isReadOnly()]);
                    }
                }
                foreach ($class->getMethods(ReflectionMethod::IS_PUBLIC) as $it) {
                    if ($it->getDeclaringClass()->getName() !== $name) {
                        continue;
                    }
                    $parameters = array_map(static fn (ReflectionParameter $p): string =>
                        $type($p->getType()) . ($p->isPassedByReference() ? ' &' : ' ')
                        . ($p->isVariadic() ? '...' : '') . '$' . $p->getName()
                        . ($p->isOptional() && !$p->isVariadic() ? ' =' : ''), $it->getParameters());
                    $lines[] = "$name::{$it->getName()}(" . implode(', ', $parameters) . '): '
                        . $type($it->getReturnType()) . ' '
                        . $flags(['static' => $it->isStatic(), 'abstract' => $it->isAbstract(),
                            'final' => $it->isFinal()]);
                }
            }
            sort($lines);
            echo implode("\\n", $lines), "\\n";
            """;

    @Test
    void theSurfaceIsTheOneItsProtocolRecorded() throws Exception {
        String protocol = Php.ran(List.of("-r", "require '"
                + RUNTIME.resolve("src").resolve("Binding.php").toAbsolutePath()
                + "'; echo \\Souther\\Runtime\\Binding::PROTOCOL;"));
        Path script = Files.createTempFile("surface", ".php");
        Files.writeString(script, "<?php\n" + SURFACE, StandardCharsets.UTF_8);
        String surface = Php.ran(List.of(script.toString(), RUNTIME.toAbsolutePath().toString()));
        Path record = RECORDED.resolve(protocol + ".txt");
        String recorded = Files.exists(record) ? Files.readString(record, StandardCharsets.UTF_8) : "";
        if (!surface.equals(recorded)) {
            // Kept where it can be compared with the record, and copied over it once it is read.
            Files.writeString(Path.of("target", "php-runtime-surface-" + protocol + ".txt"), surface,
                    StandardCharsets.UTF_8);
        }

        assertThat(surface)
                .as("the runtime's surface is not the one protocol %s recorded: a binding written"
                        + " against one of the two would load over the other. Move the protocol"
                        + " (Binding::MOVES, Binding::PROTOCOL, PhpBindings.RUNTIME_PROTOCOL) and"
                        + " record target/php-runtime-surface-<n>.txt under the new number,"
                        + " leaving the records before it as they are", protocol)
                .isEqualTo(recorded);
    }
}
