package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value whose declarations nest a million deep, written at a boundary and by a type's own
 * encoder, on the stack a process's main thread starts with.
 *
 * <p>What is asked is whether writing a value takes a native frame for each level it nests. One
 * that did would end this run by overflowing the stack, a long way short of a million. The two
 * values go down through every kind of declaration a writer walks into: a type built from fields,
 * an optional field, a newtype, and a sum whose case is chosen by its token.
 *
 * <p>The values are built by the harness, one level at a time through the functions the object
 * offers a host, and not by a behavior. A behavior that built them would be asking how deep a
 * construction goes as well, and that is not what this is about. For the same reason what was
 * written is held against what the harness spells out, byte for byte, and not read by a JSON
 * reader, which would be asked how deep it reads.
 */
class AValueIsWrittenHoweverDeeplyItsDeclarationsNestTest {

    private static final String DEEP = """
            module deep exposing ( Chain, Nest, Deeper, Bottom, Held, sameChain, sameHeld )

            data Chain = { n: Int, next: Chain? }

            data Bottom
            data Deeper = { inner: Held }
            data Nest = Deeper | Bottom
            data Held = Nest

            behavior sameChain : (chain: Chain) -> Chain
            let sameChain (chain) = chain

            behavior sameHeld : (held: Held) -> Held
            let sameHeld (held) = held
            """;

    @Test
    void aValueAMillionLevelsDeepIsWrittenWhole() throws Exception {
        Path run = NativeArtifacts.executable(Checked.of(List.of(DEEP)), List.of(), harness());
        Process process = new ProcessBuilder(run.toString()).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        assertThat(process.waitFor()).as(said).isZero();

        assertThat(said).isEqualTo("""
                chain at its boundary: status 0, as spelt 1
                chain by its encoder: as spelt 1
                held at its boundary: status 0, as spelt 1
                held by its encoder: as spelt 1
                """);
    }

    private static String host(String operation) {
        int at = operation.indexOf('$');
        return Running.hostSymbol("deep", operation.substring(0, at), operation.substring(at + 1));
    }

    private static String boundary(String name) {
        return Running.PREFIX + "souther" + Running.ABI + ".deep." + name + "$boundary";
    }

    private static String harness() {
        return """
                #include <stdint.h>
                #include <stdio.h>
                #include <stdlib.h>
                #include <string.h>

                typedef const void *Value;

                extern int64_t souther_mark(void);
                extern void souther_reset(int64_t);
                extern int64_t souther_string_length(Value);
                extern const uint8_t *souther_string_bytes(Value);

                extern uint32_t chain(int64_t, uint8_t, Value, Value *) __asm__("%1$s");
                extern Value chainEncoded(Value) __asm__("%2$s");
                extern uint32_t bottom(Value *) __asm__("%3$s");
                extern uint32_t deeper(Value, Value *) __asm__("%4$s");
                extern uint32_t held(Value, Value *) __asm__("%5$s");
                extern Value heldEncoded(Value) __asm__("%6$s");
                extern uint32_t sameChain(const void *, Value, Value *) __asm__("%7$s");
                extern uint32_t sameHeld(const void *, Value, Value *) __asm__("%8$s");

                enum { DEPTH = 1000000 };

                /* What a value is written as, spelt out here one level at a time. */
                typedef struct {
                    char *bytes;
                    size_t length;
                    size_t room;
                } Spelt;

                static void spell(Spelt *spelt, const char *text) {
                    size_t more = strlen(text);
                    if (spelt->length + more > spelt->room) {
                        spelt->room = (spelt->length + more) * 2;
                        spelt->bytes = realloc(spelt->bytes, spelt->room);
                    }
                    memcpy(spelt->bytes + spelt->length, text, more);
                    spelt->length += more;
                }

                static int asSpelt(Value written, const Spelt *spelt) {
                    return (size_t) souther_string_length(written) == spelt->length
                        && memcmp(souther_string_bytes(written), spelt->bytes, spelt->length) == 0;
                }

                int main(void) {
                    int64_t mark = souther_mark();

                    /* Built from the innermost level out, so the last one built is the outermost. */
                    Value outermost = 0;
                    for (int64_t n = DEPTH - 1; n >= 0; n--) {
                        Value next = outermost;
                        chain(n, next != 0, next, &outermost);
                    }
                    Spelt chainSpelt = {0};
                    char number[32];
                    for (int64_t n = 0; n < DEPTH; n++) {
                        snprintf(number, sizeof number, "{\\"n\\":%%lld", (long long) n);
                        spell(&chainSpelt, number);
                        if (n < DEPTH - 1) {
                            spell(&chainSpelt, ",\\"next\\":");
                        }
                    }
                    for (int64_t n = 0; n < DEPTH; n++) {
                        spell(&chainSpelt, "}");
                    }
                    Value written = 0;
                    uint32_t status = sameChain(NULL, outermost, &written);
                    printf("chain at its boundary: status %%u, as spelt %%d\\n", status,
                           asSpelt(written, &chainSpelt));
                    printf("chain by its encoder: as spelt %%d\\n",
                           asSpelt(chainEncoded(outermost), &chainSpelt));

                    Value nest = 0;
                    bottom(&nest);
                    Value wrapped = 0;
                    held(nest, &wrapped);
                    for (int64_t n = 0; n < DEPTH; n++) {
                        deeper(wrapped, &nest);
                        held(nest, &wrapped);
                    }
                    Spelt heldSpelt = {0};
                    for (int64_t n = 0; n < DEPTH; n++) {
                        spell(&heldSpelt, "{\\"type\\":\\"Deeper\\",\\"inner\\":");
                    }
                    spell(&heldSpelt, "{\\"type\\":\\"Bottom\\"}");
                    for (int64_t n = 0; n < DEPTH; n++) {
                        spell(&heldSpelt, "}");
                    }
                    written = 0;
                    status = sameHeld(NULL, wrapped, &written);
                    printf("held at its boundary: status %%u, as spelt %%d\\n", status,
                           asSpelt(written, &heldSpelt));
                    printf("held by its encoder: as spelt %%d\\n",
                           asSpelt(heldEncoded(wrapped), &heldSpelt));

                    souther_reset(mark);
                    return 0;
                }
                """.formatted(
                host("Chain$construct"), host("Chain$encode"),
                host("Bottom$construct"), host("Deeper$construct"),
                host("Held$construct"), host("Held$encode"),
                boundary("sameChain"), boundary("sameHeld"));
    }
}
