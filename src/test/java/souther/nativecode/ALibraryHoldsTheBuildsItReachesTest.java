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

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A program that reaches a type another build declares is built for a host with that build's
 * object in it: a value of the type is built and read by that object, so a library without it is
 * a program missing part of itself.
 *
 * <p>What the library offers a host is what each object in it carries, so the other build's types
 * are declared in the header beside this program's, and a host builds a value of one of them to
 * hand to this program's behavior. Nothing here says what the other build offers: its object
 * does.
 */
class ALibraryHoldsTheBuildsItReachesTest {

    private static final String BUILT_BEFORE = """
            module lib.money exposing ( Money, Price, Open, Closed, Door, Settled )

            data Money = Int
                invariant notNegative = value >= 0

            data Price = { amount: Money }

            data Closed
            data Open = { since: Money }
            data Door = Open | Closed

            data Waived = { reason: Money }
            data Settled = Closed | Waived
            """;

    private static final String BUILDING = """
            module app.order exposing ( Order, total )
            import lib.money ( Money, Price, Door, Settled )

            data Order = { price: Price, door: Door, settled: Settled, count: Int }
                invariant counted = count > 0

            behavior total : (order: Order) -> Int
            let total (order) = order.price.amount.value * order.count
            """;

    private static final String HARNESS = """
            #include <inttypes.h>
            #include <stdio.h>
            #include <string.h>
            #include "souther.h"

            static void text(souther_string said) {
                printf("%.*s", (int) souther_string_length(said),
                       (const char *) souther_string_bytes(said));
            }

            int main(void) {
                int64_t mark = souther_mark();

                souther_value three = NULL, price = NULL, closed = NULL, order = NULL;
                souther4_m_lib_m_money_t_Money_construct(3, &three);
                souther4_m_lib_m_money_t_Price_construct(three, &price);
                souther4_m_lib_m_money_t_Closed_construct(&closed);
                souther_status status =
                        souther4_m_app_m_order_t_Order_construct(price, closed, closed, 2, &order);
                int64_t total = -1;
                souther_status totalled = souther4_m_app_m_order_b_total(NULL, order, &total);
                printf("order: status %u, total %u %" PRId64 ", door %u\\n", status, totalled,
                       total, souther4_m_lib_m_money_t_Door_case(
                               souther4_m_app_m_order_t_Order_f_door(order)));
                printf("written: ");
                text(souther4_m_app_m_order_t_Order_encode(order));
                printf("\\n");

                const char *json = "{\\"price\\":{\\"amount\\":-1},\\"door\\":{\\"type\\":\\"Closed\\"},"
                        "\\"settled\\":{\\"type\\":\\"Closed\\"},\\"count\\":1}";
                souther_decoded reading = NULL;
                souther4_m_app_m_order_t_Order_decode((const uint8_t *) json, (int64_t) strlen(json),
                                                      &reading);
                souther_issue issue = souther_decoded_issue(reading, 0);
                printf("read: ");
                text(souther_issue_path(issue));
                printf(" ");
                text(souther_issue_code(issue));
                printf("\\n");

                souther_reset(mark);
                return 0;
            }
            """;

    @Test
    void aHostReachesBothBuildsThroughOneLibrary(@TempDir Path into) throws Exception {
        byte[] before = NativeArtifacts.object(CheckedProgram.of(List.of(BUILT_BEFORE)));
        NativeCompiler.Library library = NativeCompiler.library(program(), List.of(before), into);

        String declarations = Files.readString(library.declarations(), StandardCharsets.UTF_8);
        assertThat(declarations)
                .contains("souther4_m_lib_m_money_t_Money_construct(")
                .contains("souther4_m_app_m_order_b_total(");

        Path source = into.resolve("host.c");
        Files.writeString(source, HARNESS, StandardCharsets.UTF_8);
        Path executable = into.resolve("host");
        said(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), source.toString(),
                "-I", into.toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));

        assertThat(said(List.of(executable.toString()))).isEqualTo("""
                order: status 0, total 0 6, door 1
                written: {"price":{"amount":3},"door":{"type":"Closed"},"settled":{"type":"Closed"},"count":2}
                read: /price/amount invariant_violation
                """);
    }

    /** Without the other build's object, the program is not all there, and the link says so. */
    @Test
    void aLibraryWithoutTheBuildItReachesIsRefused(@TempDir Path into) {
        assertThatThrownBy(
                        () -> NativeCompiler.library(program(), List.of(), into))
                .hasMessageContaining("the linker did not make");
    }

    private static CheckedProgram program() {
        Map<String, ClassFileImage> published = Compiler.compile(BUILT_BEFORE);
        return CheckedProgram.of(List.of(BUILDING), ModulePath.of(published));
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
