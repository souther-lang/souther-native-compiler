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
 * the program: the host makes a capability of an implementation of its own through the object of
 * the build that declares the behavior, and hands it, among the requirements, to what requires the
 * behavior. Nothing is registered anywhere, so two implementations of one behavior are two
 * capabilities, each reached by what it was handed to.
 *
 * <p>So a library of such a program links with nothing but Souther objects and the runtime, and a
 * host language that can hand C a function pointer implements one in its own language. What the
 * host makes a capability through is declared in the header, described in the manifest and
 * exported by the library, the same as everything else a host reaches.
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
     * A binding's shape, written by hand: the implementation C calls is made into a C function
     * pointer once, and each implementation a host hands over is a capability of that pointer and a
     * number of its own, which the pointer is handed first and reads which implementation it is by.
     * An exception thrown by an implementation is kept, answered as a status, and thrown again
     * where the outermost call returns: PHP cannot throw through a C frame, and nothing unwinds
     * through generated code anyway.
     *
     * <p>Made into a pointer once and not handed over as a closure each time: PHP makes a new C
     * entry for a closure every time one is handed to C as a function pointer, and keeps each
     * until the request ends, so a binding handing its closures over as they come grows for as long
     * as the process lives. The script holds itself to that.
     */
    private static final String PHP = """
            <?php
            $ffi = FFI::cdef(file_get_contents($argv[1]), $argv[2]);

            final class Pending {
                public static ?Throwable $thrown = null;
            }

            /** Every implementation a capability was made of, by the number it is handed. */
            final class Implementations {
                public static array $by = [];
                public static int $next = 0;
                public static ?FFI\\CData $pointer = null;
            }

            /** The implementation as C calls it, made once and held for as long as it is used. */
            function pointer(FFI $ffi): FFI\\CData {
                if (Implementations::$pointer === null) {
                    $held = $ffi->new("souther4_m_pricing_b_lookUp_implementation[1]");
                    $held[0] = function ($by, int $a, $out) use ($ffi): int {
                        try {
                            $out[0] = (Implementations::$by[$ffi->cast("int64_t *", $by)[0]])($a);
                            return $ffi->SOUTHER_ANSWERED;
                        } catch (Throwable $thrown) {
                            Pending::$thrown = $thrown;
                            return $ffi->SOUTHER_HOST_EXCEPTION;
                        }
                    };
                    Implementations::$pointer = $held[0];
                }
                return Implementations::$pointer;
            }

            /**
             * What `twice` is called with to reach `$implementation`, and what that is made of, all
             * held for as long as it is used.
             */
            function implementing(FFI $ffi, callable $implementation): array {
                $number = $ffi->new("int64_t");
                $number->cdata = ++Implementations::$next;
                Implementations::$by[$number->cdata] = $implementation;
                $capability = $ffi->new("souther_capability");
                $hosted = $ffi->new("souther_hosted");
                $ffi->souther4_m_pricing_b_lookUp_implement(
                        FFI::addr($capability), FFI::addr($hosted), pointer($ffi), FFI::addr($number));
                $requirements = $ffi->new("const souther_capability *[1]");
                $requirements[0] = FFI::addr($capability);
                return [$requirements, $capability, $hosted, $number];
            }

            function twice(FFI $ffi, ?array $with, int $a): int {
                $answer = $ffi->new("int64_t");
                $status = $ffi->souther4_m_pricing_b_twice($with[0] ?? null, $a, FFI::addr($answer));
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

            try {
                twice($ffi, null, 1);
            } catch (RuntimeException $unbound) {
                echo "nothing: ", $unbound->getMessage() === "status " . $ffi->SOUTHER_INJECTION_UNBOUND
                        ? "unbound" : $unbound->getMessage(), "\\n";
            }

            $added = implementing($ffi, fn(int $a): int => $a + 20);
            $other = implementing($ffi, fn(int $a): int => $a + 30);
            echo "added: ", twice($ffi, $added, 1), "\\n";
            echo "other: ", twice($ffi, $other, 1), "\\n";
            echo "again: ", twice($ffi, $added, 1), "\\n";

            $down = new LogicException("the database is down");
            $throwing = implementing($ffi, function (int $a) use ($down): int { throw $down; });
            try {
                twice($ffi, $throwing, 1);
            } catch (LogicException $caught) {
                echo "thrown: ", $caught === $down ? "the same one" : "another", "\\n";
            }

            // One implementation calling the program with another inside it, and then with itself:
            // each call reaches the implementation it was handed, whatever is going around it.
            $inner = implementing($ffi, fn(int $a): int => $a + 1);
            $outer = null;
            $outer = implementing($ffi, function (int $a) use ($ffi, $inner, &$outer): int {
                if ($a === 2) {
                    return 100;
                }
                return twice($ffi, $inner, 5) + twice($ffi, $outer, 2);
            });
            echo "nested: ", twice($ffi, $outer, 1), "\\n";

            // A capability made for each of ten thousand calls, and PHP holds no more than it did. A
            // closure handed over as each is made would hold a C entry for every one of them, which
            // is megabytes, well past what PHP's own allocator moves by.
            $adding = fn(int $a): int => $a + 20;
            $before = memory_get_usage();
            for ($call = 0; $call < 10000; $call++) {
                $made = implementing($ffi, $adding);
                twice($ffi, $made, $call);
                unset(Implementations::$by[$made[3]->cdata], $made);
            }
            $grown = memory_get_usage() - $before;
            echo "repeated: ", $grown < 64 * 1024 ? "steady" : "grew $grown bytes", "\\n";
            """;

    @Test
    void phpImplementsABehaviorWithAClosure(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(PRICING)), into);

        assertThat(Files.readString(library.declarations(), StandardCharsets.UTF_8))
                .contains("typedef souther_status (*souther4_m_pricing_b_lookUp_implementation)"
                        + "(void *, int64_t, int64_t *);")
                .contains("void souther4_m_pricing_b_lookUp_implement(souther_capability *, "
                        + "souther_hosted *, souther4_m_pricing_b_lookUp_implementation, void *);");

        Path script = into.resolve("host.php");
        Files.writeString(script, PHP, StandardCharsets.UTF_8);
        assertThat(Php.ran(List.of("-d", "ffi.enable=1", script.toString(),
                library.declarations().toString(), library.library().toString())))
                .isEqualTo("""
                        nothing: unbound
                        added: 42
                        other: 62
                        again: 42
                        thrown: the same one
                        nested: 424
                        repeated: steady
                        """);
    }

    private static final String SHOP = """
            module shop exposing ( Money, quote, cheap )

            data Money = Int
                invariant notNegative = value >= 0

            behavior priceOf : (sku: String, gift: Bool) -> Money

            behavior isCheap : (price: Money) -> Bool

            behavior quote : (sku: String, gift: Bool, count: Int) -> Int
                depends on priceOf
            let quote (sku, gift, count, priceOf) = priceOf(sku, gift).value * count

            behavior cheap : (n: Int) -> Bool
                depends on isCheap
            let cheap (n, isCheap) = isCheap(Money(n))
            """;

    /**
     * Implementations handed text, a truth and a value of a declared type, answering a value and a
     * truth: every word a behavior's crossing is made of besides a number, each way. One builds its
     * answer with the type's own host constructor, and where that refuses the value, what it hands
     * back is not an answer the model can end with.
     */
    private static final String WORDS = """
            #include <stdio.h>
            #include <string.h>
            #include "souther.h"

            static souther_status priced(void *by, souther_string sku, uint8_t gift, souther_value *out) {
                int64_t cents = strncmp((const char *) souther_string_bytes(sku), "free", 4) == 0
                        ? -1 : souther_string_length(sku) * 100 + (gift ? 50 : 0);
                return souther4_m_shop_t_Money_construct(cents, out);
            }

            static souther_status judged(void *by, souther_value price, uint8_t *out) {
                *out = souther4_m_shop_t_Money_f_value(price) < 300;
                return SOUTHER_ANSWERED;
            }

            static const souther_capability *priceOf[1];
            static const souther_capability *isCheap[1];

            static int64_t quoted(const char *sku, uint8_t gift, int64_t count, souther_status *status) {
                souther_string text = souther_string_of_utf8((const uint8_t *) sku, (int64_t) strlen(sku));
                int64_t answer = -1;
                *status = souther4_m_shop_b_quote(priceOf, text, gift, count, &answer);
                return answer;
            }

            int main(void) {
                int64_t mark = souther_mark();
                souther_capability pricing, judging;
                souther_hosted priced_by, judged_by;
                souther4_m_shop_b_priceOf_implement(&pricing, &priced_by, priced, NULL);
                souther4_m_shop_b_isCheap_implement(&judging, &judged_by, judged, NULL);
                priceOf[0] = &pricing;
                isCheap[0] = &judging;
                souther_status status;
                int64_t answer = quoted("abc", 1, 2, &status);
                printf("gift %u %lld\\n", status, (long long) answer);
                answer = quoted("ab", 0, 1, &status);
                printf("plain %u %lld\\n", status, (long long) answer);
                answer = quoted("free", 0, 1, &status);
                printf("refused %d %lld\\n", status == SOUTHER_INJECTION_PROTOCOL_VIOLATION,
                       (long long) answer);
                uint8_t cheap = 9;
                status = souther4_m_shop_b_cheap(isCheap, 250, &cheap);
                printf("cheap %u %u\\n", status, cheap);
                status = souther4_m_shop_b_cheap(isCheap, 400, &cheap);
                printf("dear %u %u\\n", status, cheap);
                souther_reset(mark);
                return 0;
            }
            """;

    @Test
    void textATruthAndAValueCrossToAnImplementationAndBack(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into);
        assertThat(Files.readString(library.declarations(), StandardCharsets.UTF_8))
                .contains("typedef souther_status (*souther4_m_shop_b_priceOf_implementation)"
                        + "(void *, souther_string, uint8_t, souther_value *);")
                .contains("typedef souther_status (*souther4_m_shop_b_isCheap_implementation)"
                        + "(void *, souther_value, uint8_t *);");

        Path source = into.resolve("host.c");
        Files.writeString(source, WORDS, StandardCharsets.UTF_8);
        Path executable = into.resolve("host");
        said(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), source.toString(),
                "-I", into.toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));
        assertThat(said(List.of(executable.toString()))).isEqualTo("""
                gift 0 700
                plain 0 200
                refused 1 -1
                cheap 0 1
                dear 0 0
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

            static souther_status added(void *by, int64_t a, int64_t *out) {
                *out = a + 20;
                return SOUTHER_ANSWERED;
            }

            int main(void) {
                souther_capability adding;
                souther_hosted added_by;
                souther4_m_lib_m_port_b_lookUp_implement(&adding, &added_by, added, NULL);
                const souther_capability *lookUp[1] = {&adding};
                int64_t twice = -1;
                int64_t thrice = -1;
                souther_status first = souther4_m_app_m_first_b_twice(lookUp, 1, &twice);
                souther_status second = souther4_m_app_m_second_b_thrice(lookUp, 1, &thrice);
                printf("%u %" PRId64 " %u %" PRId64 "\\n", first, twice, second, thrice);
                return 0;
            }
            """;

    /**
     * Two builds requiring one behavior with no body, and the build that declares it: the declaring
     * build's object makes the capability a host's implementation is handed over as, and the others
     * only call through one. So the library holds one function a host makes it through, however
     * many objects reach it, and nothing defines the behavior under a symbol of its own.
     */
    @Test
    void theBuildThatDeclaresItAnswersItForEveryBuildThatCallsIt(@TempDir Path into)
            throws Exception {
        Map<String, ClassFileImage> published = Compiler.compile(PORT);
        byte[] port = NativeArtifacts.object(Checked.of(List.of(PORT)));
        byte[] second = NativeArtifacts.object(
                Checked.of(List.of(SECOND), ModulePath.of(published)));
        CheckedProgram first = Checked.of(List.of(FIRST), ModulePath.of(published));

        String symbol = "souther" + Running.ABI + ".lib.port.lookUp";
        String implement = "souther" + Running.ABI + "_m_lib_m_port_b_lookUp_implement";
        assertThat(symbols(port, false)).contains(implement).doesNotContain(symbol);
        assertThat(symbols(second, true)).doesNotContain(symbol, implement);
        assertThat(symbols(second, false)).doesNotContain(symbol, implement);

        NativeCompiler.Library library =
                NativeCompiler.library(first, List.of(port, second), into);
        assertThat(symbols(Files.readAllBytes(library.object()), true))
                .doesNotContain(symbol, implement);
        String declarations = Files.readString(library.declarations(), StandardCharsets.UTF_8);
        assertThat(declarations.split(implement + "\\(", -1)).hasSize(2);

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
