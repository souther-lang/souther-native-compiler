package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.program.CheckedProgram;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A behavior with no body that declares nothing to depend on is implemented by the host that runs
 * the program, when it runs it: the host registers a function for it on the thread it calls from,
 * and the object of the build that declares the behavior calls that function.
 *
 * <p>So a library of such a program links with nothing but Souther objects and the runtime, and a
 * host language that can hand C a function pointer implements one in its own language. What the
 * host registers through is declared in the header, described in the manifest and exported by the
 * library, the same as everything else a host reaches.
 */
class AHostImplementsABehaviorWithNoBodyTest {

    private static final String PRICING = """
            module pricing exposing ( twice )

            behavior lookUp : (a: Int) -> Int

            behavior twice : (a: Int) -> Int
                depends on lookUp
            let twice (a, lookUp) = lookUp(a) * 2
            """;

    /**
     * A binding's shape, written by hand: an implementation is made into a C function pointer
     * once, registered around each call it is for, and what it replaced is put back after. An
     * exception thrown by an implementation is kept, answered as a status, and thrown again where
     * the outermost call returns: PHP cannot throw through a C frame, and nothing unwinds through
     * generated code anyway.
     *
     * <p>Made into a pointer once and not handed over as a closure each call: PHP makes a new C
     * entry for a closure every time one is handed to C as a function pointer, and keeps each
     * until the request ends, so a binding handing its closure over on every call grows for as
     * long as the process lives. The script holds itself to that.
     */
    private static final String PHP = """
            <?php
            $ffi = FFI::cdef(file_get_contents($argv[1]), $argv[2]);

            final class Pending {
                public static ?Throwable $thrown = null;
            }

            /** The implementation as C calls it, made once and held for as long as it is used. */
            function implementing(FFI $ffi, callable $implementation): FFI\\CData {
                $held = $ffi->new("souther3_m_pricing_b_lookUp_implementation[1]");
                $held[0] = function (int $a, $out) use ($ffi, $implementation): int {
                    try {
                        $out[0] = $implementation($a);
                        return $ffi->SOUTHER_ANSWERED;
                    } catch (Throwable $thrown) {
                        Pending::$thrown = $thrown;
                        return $ffi->SOUTHER_HOST_EXCEPTION;
                    }
                };
                return $held[0];
            }

            function twice(FFI $ffi, int $a): int {
                $answer = $ffi->new("int64_t");
                $status = $ffi->souther3_m_pricing_b_twice($a, FFI::addr($answer));
                if ($status === $ffi->SOUTHER_HOST_EXCEPTION) {
                    $thrown = Pending::$thrown;
                    Pending::$thrown = null;
                    throw $thrown;
                }
                if ($status !== $ffi->SOUTHER_ANSWERED) {
                    throw new RuntimeException("status $status");
                }
                return $answer->cdata;
            }

            function bound(FFI $ffi, FFI\\CData $implementation, int $a): int {
                $before = $ffi->souther3_m_pricing_b_lookUp_register($implementation);
                try {
                    return twice($ffi, $a);
                } finally {
                    $ffi->souther3_m_pricing_b_lookUp_register($before);
                }
            }

            try {
                twice($ffi, 1);
            } catch (RuntimeException $unbound) {
                echo "nothing: ", $unbound->getMessage() === "status " . $ffi->SOUTHER_INJECTION_UNBOUND
                        ? "unbound" : $unbound->getMessage(), "\\n";
            }

            $added = implementing($ffi, fn(int $a): int => $a + 20);
            echo "added: ", bound($ffi, $added, 1), "\\n";

            $down = new LogicException("the database is down");
            $throwing = implementing($ffi, function (int $a) use ($down): int { throw $down; });
            try {
                bound($ffi, $throwing, 1);
            } catch (LogicException $caught) {
                echo "thrown: ", $caught === $down ? "the same one" : "another", "\\n";
            }

            // One implementation calling the program with another bound inside it, and then again
            // with nothing bound anew: the second call is answered by the first implementation.
            $inner = implementing($ffi, fn(int $a): int => $a + 1);
            $outer = implementing($ffi, function (int $a) use ($ffi, $inner): int {
                if ($a === 2) {
                    return 100;
                }
                return bound($ffi, $inner, 5) + twice($ffi, 2);
            });
            echo "nested: ", bound($ffi, $outer, 1), "\\n";

            try {
                twice($ffi, 1);
            } catch (RuntimeException $unbound) {
                echo "after: ", $unbound->getMessage() === "status " . $ffi->SOUTHER_INJECTION_UNBOUND
                        ? "unbound" : $unbound->getMessage(), "\\n";
            }

            // Registered around ten thousand calls, and PHP holds no more than it did. A closure
            // handed over on each call would hold a C entry for every one of them, which is
            // megabytes, well past what PHP's own allocator moves by.
            $before = memory_get_usage();
            for ($call = 0; $call < 10000; $call++) {
                bound($ffi, $added, $call);
            }
            $grown = memory_get_usage() - $before;
            echo "repeated: ", $grown < 64 * 1024 ? "steady" : "grew $grown bytes", "\\n";
            """;

    @Test
    void phpImplementsABehaviorWithAClosure(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(PRICING)), into);

        assertThat(Files.readString(library.declarations(), StandardCharsets.UTF_8))
                .contains("typedef souther_status (*souther3_m_pricing_b_lookUp_implementation)"
                        + "(int64_t, int64_t *);")
                .contains("souther3_m_pricing_b_lookUp_implementation "
                        + "souther3_m_pricing_b_lookUp_register("
                        + "souther3_m_pricing_b_lookUp_implementation);");

        Path script = into.resolve("host.php");
        Files.writeString(script, PHP, StandardCharsets.UTF_8);
        assertThat(said(List.of("php", "-d", "ffi.enable=1", script.toString(),
                library.declarations().toString(), library.library().toString())))
                .isEqualTo("""
                        nothing: unbound
                        added: 42
                        thrown: the same one
                        nested: 424
                        after: unbound
                        repeated: steady
                        """);
    }

    private static final String PORT = """
            module lib.port exposing ( lookUp )

            behavior lookUp : (a: Int) -> Int
            """;

    private static final String FIRST = """
            module app.first exposing ( twice )
            import lib.port ( lookUp )

            behavior twice : (a: Int) -> Int
                depends on lookUp
            let twice (a, lookUp) = lookUp(a) * 2
            """;

    private static final String SECOND = """
            module app.second exposing ( thrice )
            import lib.port ( lookUp )

            behavior thrice : (a: Int) -> Int
                depends on lookUp
            let thrice (a, lookUp) = lookUp(a) * 3
            """;

    private static final String BOTH = """
            #include <inttypes.h>
            #include <stdio.h>
            #include "souther.h"

            static souther_status added(int64_t a, int64_t *out) {
                *out = a + 20;
                return SOUTHER_ANSWERED;
            }

            int main(void) {
                souther3_m_lib_m_port_b_lookUp_register(added);
                int64_t twice = -1;
                int64_t thrice = -1;
                souther_status first = souther3_m_app_m_first_b_twice(1, &twice);
                souther_status second = souther3_m_app_m_second_b_thrice(1, &thrice);
                printf("%u %" PRId64 " %u %" PRId64 "\\n", first, twice, second, thrice);
                return 0;
            }
            """;

    /**
     * Two builds calling one behavior with no body, and the build that declares it: the declaring
     * build's object answers it, and the others only call it. So the library holds one definition
     * of it and one function a host registers through, however many objects reach it.
     */
    @Test
    void theBuildThatDeclaresItAnswersItForEveryBuildThatCallsIt(@TempDir Path into)
            throws Exception {
        Map<String, ClassFileImage> published = Compiler.compile(PORT);
        byte[] port = NativeArtifacts.object(CheckedProgram.of(List.of(PORT)));
        byte[] second = NativeArtifacts.object(
                CheckedProgram.of(List.of(SECOND), ModulePath.of(published)));
        CheckedProgram first = CheckedProgram.of(List.of(FIRST), ModulePath.of(published));

        String symbol = "souther" + Running.ABI + ".lib.port.lookUp";
        String register = "souther" + Running.ABI + "_m_lib_m_port_b_lookUp_register";
        assertThat(symbols(port, false)).contains(symbol, register);
        assertThat(symbols(second, true)).contains(symbol);
        assertThat(symbols(second, false)).doesNotContain(symbol, register);

        NativeCompiler.Library library =
                NativeCompiler.library(first, List.of(port, second), into);
        assertThat(symbols(Files.readAllBytes(library.object()), true)).contains(symbol);
        String declarations = Files.readString(library.declarations(), StandardCharsets.UTF_8);
        assertThat(declarations.split(register + "\\(", -1)).hasSize(2);

        Path source = into.resolve("host.c");
        Files.writeString(source, BOTH, StandardCharsets.UTF_8);
        Path executable = into.resolve("host");
        said(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), source.toString(),
                "-I", into.toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));
        assertThat(said(List.of(executable.toString()))).isEqualTo("0 42 0 63\n");
    }

    /** What an object defines, or, with {@code undefined}, what it names and leaves to another. */
    private static Set<String> symbols(byte[] object, boolean undefined) throws Exception {
        Path file = Files.createTempFile("souther-symbols", ".o");
        try {
            Files.write(file, object);
            Set<String> named = new TreeSet<>();
            for (String line : said(List.of("nm", undefined ? "-u" : "-g", file.toString()))
                    .lines().toList()) {
                String[] fields = line.trim().split("\\s+");
                String name = fields[fields.length - 1];
                if (!undefined && (fields.length != 3 || fields[1].equals("U"))) {
                    continue;
                }
                named.add(name.startsWith(Running.PREFIX)
                        ? name.substring(Running.PREFIX.length()) : name);
            }
            return named;
        } finally {
            Files.delete(file);
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
