package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;

import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A host builds a value of a model's own type, hands it to a behavior, and reads what it is
 * answered, through functions the object defines — and never through where anything is kept.
 *
 * <p>The harness is C, the way a host binding is, and what it may not say is part of what is
 * asserted: no slot, no offset, no token. A test that read a field at an offset would pass for as
 * long as the layout it copied stayed put, which is the dependency the functions exist to remove.
 *
 * <p>A value built here and a value a behavior built are read by the same readers, so what is
 * asked is whether the two are one kind of value, and not only whether each side can read its own.
 */
class AHostReachesAValueWithoutItsLayoutTest {

    private static final String SHOP = """
            module shop exposing ( Money, Line, Free, Paid, Owed, Settled, Outcome, settle, owing )

            data Money = Int
                invariant notNegative = value >= 0

            data Line = { price: Money, quantity: Int, note: String?, gift: Bool? }
                invariant some = quantity > 0

            data Free
            data Paid = { amount: Money }
            data Waived = { reason: String }
            data Owed = { amount: Money, overdue: Bool }
            data Settled = Free | Paid | Waived
            data Outcome = Settled | Owed

            behavior settle : (line: Line, paid: Int) -> Outcome
            let settle (line, paid) = {
                let due = line.price.value * line.quantity
                if due == 0 then Free
                else if paid >= due then Paid { amount = Money(due) }
                else Owed { amount = Money(due - paid), overdue = paid == 0 }
            }

            behavior owing : (outcome: Outcome) -> Int
            let owing (outcome) = match outcome with
                | Owed as o -> o.amount.value
                | Settled -> 0
            """;

    /** What the harness may not say, since each is how this backend keeps a value. */
    private static final List<String> LAYOUT =
            List.of("SLOT", "WHICH", "HELD", "NOTHING", "field_at", "souther$type$");

    @Test
    void aHostBuildsHandsOverAndReadsAValueThroughTheObject() throws Exception {
        String harness = harness();
        for (String layout : LAYOUT) {
            assertThat(harness).as("the harness says nothing of the layout").doesNotContain(layout);
        }

        Path run = NativeArtifacts.executable(CheckedProgram.of(List.of(SHOP)), List.of(), harness);
        Process process = new ProcessBuilder(run.toString()).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        assertThat(process.waitFor()).as(said).isZero();

        assertThat(said).isEqualTo("""
                money 3: status 0, value 3
                money -1: status 1, untouched 1
                line: status 0
                line of none: status 1, untouched 1
                quantity 2, price 3
                note: present 1, text gift wrap
                gift: present 0, untouched 1
                paid in full: status 0, case 1, settled 1, amount 6, owing 0
                paid nothing: status 0, case 3, amount 6, overdue 1, owing 6
                paid some: status 0, case 3, amount 4, overdue 0, owing 4
                priced at nought: status 0, case 0, settled 0
                free from here: status 0, case 0, owing 0
                gift given: present 1, gift 1
                """);
    }

    /**
     * What a host reaches is what the module publishes. A sum it publishes is asked which case a
     * value is, a case the module keeps among them; the kept case is neither built nor read by a
     * host, whichever published sum it is a case of.
     */
    @Test
    void aHostReachesWhatTheModulePublishesAndNothingItKeeps() throws Exception {
        Set<String> defined = NativeArtifacts.built(CheckedProgram.of(List.of(SHOP))).defined();

        assertThat(defined).contains(
                host("Money$construct"), host("Money$field$value"),
                host("Line$construct"), host("Line$field$price"), host("Line$field$note"),
                host("Free$construct"), host("Settled$case"), host("Outcome$case"));
        assertThat(defined).doesNotContain(
                host("Waived$construct"), host("Waived$field$reason"));
    }

    private static String host(String operation) {
        return Running.PREFIX + "souther" + Running.ABI + ".shop$type$" + operation;
    }

    private static String behavior(String name) {
        return Running.PREFIX + "souther" + Running.ABI + ".shop." + name;
    }

    private static String harness() {
        return """
                #include <stdint.h>
                #include <stdio.h>
                #include <string.h>

                typedef const void *Value;

                extern int64_t souther_mark(void);
                extern void souther_reset(int64_t);
                extern Value souther_string_of_utf8(const uint8_t *, int64_t);
                extern int64_t souther_string_length(Value);
                extern const uint8_t *souther_string_bytes(Value);

                extern uint32_t money(int64_t, Value *) __asm__("%1$s");
                extern int64_t moneyValue(Value) __asm__("%2$s");
                extern uint32_t line(Value, int64_t, uint8_t, Value, uint8_t, uint8_t, Value *)
                        __asm__("%3$s");
                extern Value linePrice(Value) __asm__("%4$s");
                extern int64_t lineQuantity(Value) __asm__("%5$s");
                extern uint8_t lineNote(Value, Value *) __asm__("%6$s");
                extern uint8_t lineGift(Value, uint8_t *) __asm__("%7$s");
                extern uint32_t free_(Value *) __asm__("%8$s");
                extern Value paidAmount(Value) __asm__("%9$s");
                extern Value owedAmount(Value) __asm__("%10$s");
                extern uint8_t owedOverdue(Value) __asm__("%11$s");
                extern uint32_t settledCase(Value) __asm__("%12$s");
                extern uint32_t outcomeCase(Value) __asm__("%13$s");
                extern uint32_t settle(Value, int64_t, Value *) __asm__("%14$s");
                extern uint32_t owing(Value, int64_t *) __asm__("%15$s");

                static Value untouched = (Value) &untouched;

                static void settled(const char *said, Value bought, int64_t paid) {
                    Value outcome = untouched;
                    uint32_t status = settle(bought, paid, &outcome);
                    uint32_t which = outcomeCase(outcome);
                    int64_t owed = -1;
                    owing(outcome, &owed);
                    printf("%%s: status %%u, case %%u", said, status, which);
                    if (which == 3) {
                        printf(", amount %%lld, overdue %%u", (long long) moneyValue(owedAmount(outcome)),
                               owedOverdue(outcome));
                    } else {
                        printf(", settled %%u", settledCase(outcome));
                        if (which == 1) {
                            printf(", amount %%lld", (long long) moneyValue(paidAmount(outcome)));
                        }
                    }
                    if (which != 0) {
                        printf(", owing %%lld", (long long) owed);
                    }
                    printf("\\n");
                }

                int main(void) {
                    int64_t mark = souther_mark();

                    Value three = untouched;
                    uint32_t status = money(3, &three);
                    printf("money 3: status %%u, value %%lld\\n", status, (long long) moneyValue(three));
                    Value below = untouched;
                    status = money(-1, &below);
                    printf("money -1: status %%u, untouched %%d\\n", status, below == untouched);

                    const char *wrap = "gift wrap";
                    Value note = souther_string_of_utf8((const uint8_t *) wrap, (int64_t) strlen(wrap));
                    Value bought = untouched;
                    status = line(three, 2, 1, note, 0, 0, &bought);
                    printf("line: status %%u\\n", status);
                    Value none = untouched;
                    status = line(three, 0, 0, 0, 0, 0, &none);
                    printf("line of none: status %%u, untouched %%d\\n", status, none == untouched);

                    printf("quantity %%lld, price %%lld\\n", (long long) lineQuantity(bought),
                           (long long) moneyValue(linePrice(bought)));
                    Value noted = untouched;
                    uint8_t present = lineNote(bought, &noted);
                    printf("note: present %%u, text %%.*s\\n", present, (int) souther_string_length(noted),
                           (const char *) souther_string_bytes(noted));
                    uint8_t gift = 7;
                    present = lineGift(bought, &gift);
                    printf("gift: present %%u, untouched %%d\\n", present, gift == 7);

                    settled("paid in full", bought, 6);
                    settled("paid nothing", bought, 0);
                    settled("paid some", bought, 2);

                    Value nought = untouched;
                    money(0, &nought);
                    Value given = untouched;
                    line(nought, 1, 0, 0, 1, 1, &given);
                    settled("priced at nought", given, 0);

                    Value free = untouched;
                    status = free_(&free);
                    int64_t owed = -1;
                    owing(free, &owed);
                    printf("free from here: status %%u, case %%u, owing %%lld\\n", status,
                           outcomeCase(free), (long long) owed);

                    gift = 7;
                    present = lineGift(given, &gift);
                    printf("gift given: present %%u, gift %%u\\n", present, gift);

                    souther_reset(mark);
                    return 0;
                }
                """.formatted(
                host("Money$construct"), host("Money$field$value"),
                host("Line$construct"), host("Line$field$price"), host("Line$field$quantity"),
                host("Line$field$note"), host("Line$field$gift"),
                host("Free$construct"),
                host("Paid$field$amount"), host("Owed$field$amount"), host("Owed$field$overdue"),
                host("Settled$case"), host("Outcome$case"),
                behavior("settle"), behavior("owing"));
    }
}
