package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A host calls a program built for it the way a C program does: it includes the header the build
 * wrote, links the shared library the build wrote, and declares nothing itself.
 *
 * <p>No {@code __asm__} label, and no symbol spelt by the test: every name the harness calls is one
 * the header declares, so a harness that compiles is one the header was enough for. And what the
 * header declares, what the manifest describes and what the library exports are asked of each of
 * them as it is, and held to be one set.
 */
class AHostCallsALibraryThroughItsHeaderTest {

    private static final String SHOP = """
            module shop exposing ( Money, Line, Free, Paid, Owed, Settled, Outcome, settle, owing, stillOwing : Int,
                                   charge, quantityOf, Basket, counted, doubled, discounted, pair, deep,
                                   pairs, either, Partial )

            data Money = Int
                invariant notNegative = value >= 0

            data Line = { price: Money, quantity: Int, note: String? }
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

            behavior stillOwing = settle >-> owing

            behavior charge : (paid: Int) -> Owed | Settled
            let charge (paid) = if paid > 0 then Free else Owed { amount = Money(1), overdue = true }

            behavior quantityOf : (paid: Int) -> Int | Free
            let quantityOf (paid) = if paid > 0 then paid else Free

            behavior twice : (n: Int) -> Int
            let twice (n) = n * 2

            data Basket = { lines: List<Line>, notes: List<Option<String>>, groups: List<List<Int>> }

            behavior counted : (lines: List<Line>) -> Int
            let counted (lines) = List.length(lines)

            behavior doubled : (line: Line) -> List<Line>
            let doubled (line) = [line, line]

            behavior discountFor : (line: Line) -> Int

            behavior discounted : (line: Line) -> Int
                depends on discountFor
            let discounted (line, discountFor) = line.price.value * line.quantity - discountFor(line)

            example twice
                | "two" : (1) -> 2

            let pair: (Int, Bool) = (3, true)

            let deep = List.get(0, [Line { price = Money(1), quantity = 1, note = None }.note])

            let pairs = [(1, "a"), (2, "b")]

            let either: Int | Free = Free

            data Partial = { count: Int, amount: Decimal }
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

            /* A field is written through room: each of these reads one into its own. */
            static int64_t quantity_of(souther_value line) {
                int64_t quantity = -1;
                souther5_m_shop_t_Line_f_quantity(line, &quantity);
                return quantity;
            }

            static int64_t value_of(souther_value money) {
                int64_t value = -1;
                souther5_m_shop_t_Money_f_value(money, &value);
                return value;
            }

            static souther_value amount_of(souther_value owed) {
                souther_value amount = NULL;
                souther5_m_shop_t_Owed_f_amount(owed, &amount);
                return amount;
            }

            static souther_list notes_of(souther_value basket) {
                souther_list notes = NULL;
                souther5_m_shop_t_Basket_f_notes(basket, &notes);
                return notes;
            }

            static souther_list groups_of(souther_value basket) {
                souther_list groups = NULL;
                souther5_m_shop_t_Basket_f_groups(basket, &groups);
                return groups;
            }

            static void decoded(const char *label, const char *json) {
                souther_decoded reading = NULL;
                souther_status status = souther5_m_shop_t_Line_decode(
                        (const uint8_t *) json, (int64_t) strlen(json), &reading);
                printf("%s: status %u", label, status);
                switch (souther_decoded_outcome(reading)) {
                case SOUTHER_DECODED_VALUE:
                    printf(", quantity %" PRId64,
                           quantity_of(souther_decoded_value(reading)));
                    break;
                case SOUTHER_DECODED_ISSUES:
                    for (int64_t at = 0; at < souther_decoded_issue_count(reading); at++) {
                        souther_issue issue = souther_decoded_issue(reading, at);
                        printf(", [");
                        text(souther_issue_path(issue));
                        printf(" ");
                        text(souther_issue_code(issue));
                        printf("]");
                    }
                    break;
                case SOUTHER_DECODED_MALFORMED:
                    printf(", malformed at %" PRId64, souther_decoded_malformed_at(reading));
                    break;
                }
                printf("\\n");
            }

            int main(void) {
                int64_t mark = souther_mark();

                souther_value three = NULL;
                souther_status status = souther5_m_shop_t_Money_construct(3, &three);
                printf("money: status %u, value %" PRId64 "\\n", status,
                       value_of(three));
                souther_value below = NULL;
                status = souther5_m_shop_t_Money_construct(-1, &below);
                printf("below: %d\\n", status == SOUTHER_INVARIANT_NOT_HELD && below == NULL);

                const char *wrap = "gift wrap";
                souther_string note = souther_string_of_utf8((const uint8_t *) wrap,
                                                             (int64_t) strlen(wrap));
                souther_value line = NULL;
                status = souther5_m_shop_t_Line_construct(three, 2, 1, note, &line);
                souther_string noted = NULL;
                uint8_t present = 9;
                souther5_m_shop_t_Line_f_note(line, &present, &noted);
                printf("line: status %u, note %u ", status, present);
                text(noted);
                printf("\\n");

                souther_value outcome = NULL;
                status = souther5_m_shop_b_settle(NULL, line, 2, &outcome);
                int64_t owed = -1;
                souther_status owing = souther5_m_shop_b_owing(NULL, outcome, &owed);
                printf("settled: status %u, case %u, amount %" PRId64 ", owing %u %" PRId64 "\\n",
                       status, souther5_m_shop_t_Outcome_case(outcome),
                       value_of(amount_of(outcome)),
                       owing, owed);

                souther_value owes = NULL;
                status = souther5_m_shop_b_charge(NULL, 0, &owes);
                souther_value free = NULL;
                souther_status freed = souther5_m_shop_b_charge(NULL, 1, &free);
                printf("charged: status %u, case %u, status %u, case %u\\n", status,
                       souther5_m_shop_b_charge_answer_case(owes), freed,
                       souther5_m_shop_b_charge_answer_case(free));

                souther_value counted_five = NULL;
                status = souther5_m_shop_b_quantityOf(NULL, 5, &counted_five);
                souther_value none_counted = NULL;
                souther_status nothing = souther5_m_shop_b_quantityOf(NULL, 0, &none_counted);
                souther_value made = souther_case_int_make(7);
                printf("counted: status %u, case %u, quantity %" PRId64 ", status %u, case %u,"
                       " made case %u quantity %" PRId64 "\\n", status,
                       souther5_m_shop_b_quantityOf_answer_case(counted_five),
                       souther_case_int_read(counted_five), nothing,
                       souther5_m_shop_b_quantityOf_answer_case(none_counted),
                       souther5_m_shop_b_quantityOf_answer_case(made), souther_case_int_read(made));

                const souther_value both[2] = {line, line};
                souther_list lines = souther5_m_shop_l_value_construct(2, both);
                souther_value second = NULL;
                uint8_t inside = souther5_m_shop_l_value_at(lines, 1, &second);
                souther_value past = NULL;
                uint8_t outside = souther5_m_shop_l_value_at(lines, 2, &past);
                uint8_t before = souther5_m_shop_l_value_at(lines, -1, &past);
                int64_t counted = -1;
                status = souther5_m_shop_b_counted(NULL, lines, &counted);
                souther_list doubled = NULL;
                souther_status twice = souther5_m_shop_b_doubled(NULL, line, &doubled);
                printf("lines: length %" PRId64 ", at 1 %u quantity %" PRId64 ", at 2 %u, at -1 %u,"
                       " untouched %d, counted %u %" PRId64 ", doubled %u %" PRId64 "\\n",
                       souther5_m_shop_l_value_length(lines), inside,
                       quantity_of(second), outside, before, past == NULL,
                       status, counted, twice, souther5_m_shop_l_value_length(doubled));

                const uint8_t there[2] = {0, 1};
                const souther_string said[2] = {NULL, note};
                souther_list notes = souther5_m_shop_l_o_string_construct(2, there, said);
                const int64_t ones[1] = {1};
                const souther_list rows[2] = {souther5_m_shop_l_int_construct(1, ones),
                                              souther5_m_shop_l_int_construct(0, NULL)};
                souther_list groups = souther5_m_shop_l_l_int_construct(2, rows);
                souther_value basket = NULL;
                status = souther5_m_shop_t_Basket_construct(lines, notes, groups, &basket);
                uint8_t first_there = 9;
                souther_string first = NULL;
                souther5_m_shop_l_o_string_at(notes_of(basket), 0,
                                                    &first_there, &first);
                uint8_t second_there = 9;
                souther_string noted_second = NULL;
                souther5_m_shop_l_o_string_at(notes_of(basket), 1,
                                                    &second_there, &noted_second);
                souther_list group = NULL;
                souther5_m_shop_l_l_int_at(groups_of(basket), 0, &group);
                int64_t one = 0;
                souther5_m_shop_l_int_at(group, 0, &one);
                printf("basket: status %u, notes %u %d %u ", status, first_there, first == NULL,
                       second_there);
                text(noted_second);
                printf(", group %" PRId64 " %" PRId64 ", written ", souther5_m_shop_l_int_length(group),
                       one);
                text(souther5_m_shop_t_Basket_encode(basket));
                printf("\\n");

                int64_t paired = -1;
                uint8_t second_of = 9;
                status = souther5_m_shop_v_pair(&paired, &second_of);
                uint8_t outer = 9, inner = 9;
                souther_string held = NULL;
                souther_status deepened = souther5_m_shop_v_deep(&outer, &inner, &held);
                printf("pair: status %u, %" PRId64 " %u, deep: status %u, %u %u %d\\n", status, paired,
                       second_of, deepened, outer, inner, held == NULL);

                souther_list pairs = NULL;
                status = souther5_m_shop_v_pairs(&pairs);
                int64_t number = -1;
                souther_string letter = NULL;
                souther5_m_shop_l_t2_int_string_at(pairs, 1, &number, &letter);
                const int64_t numbers[2] = {7, 8};
                const souther_string letters[2] = {note, note};
                souther_list built = souther5_m_shop_l_t2_int_string_construct(2, numbers, letters);
                int64_t eight = -1;
                souther_string noted_eight = NULL;
                souther5_m_shop_l_t2_int_string_at(built, 1, &eight, &noted_eight);
                printf("pairs: status %u, length %" PRId64 ", at 1 %" PRId64 " ", status,
                       souther5_m_shop_l_t2_int_string_length(pairs), number);
                text(letter);
                printf(", made %" PRId64 " ", eight);
                text(noted_eight);
                printf("\\n");

                printf("written: ");
                text(souther5_m_shop_t_Line_encode(line));
                printf("\\n");
                decoded("read", "{\\"price\\": 4, \\"quantity\\": 5}");
                decoded("read wrong", "{\\"price\\": -1, \\"quantity\\": 5}");
                decoded("not json", "{\\"price\\"");

                souther_reset(mark);
                return 0;
            }
            """;

    /**
     * The same, from PHP: the declarations handed to {@code FFI::cdef} as they were written, which
     * is the reader with no preprocessor the declarations are for.
     */
    private static final String PHP = """
            <?php
            $ffi = FFI::cdef(file_get_contents($argv[1]), $argv[2]);

            function bytes($ffi, string $text) {
                $held = $ffi->new("uint8_t[" . max(1, strlen($text)) . "]");
                FFI::memcpy($held, $text, strlen($text));
                return $held;
            }

            function text($ffi, $string): string {
                return FFI::string($ffi->souther_string_bytes($string),
                        $ffi->souther_string_length($string));
            }

            function field($ffi, string $reader, $value, string $type) {
                $room = $ffi->new($type);
                $ffi->$reader($value, FFI::addr($room));
                return $room;
            }

            function decoded($ffi, string $label, string $json): void {
                $reading = $ffi->new("souther_decoded");
                $status = $ffi->souther5_m_shop_t_Line_decode(bytes($ffi, $json), strlen($json),
                        FFI::addr($reading));
                echo "$label: status $status";
                $outcome = $ffi->souther_decoded_outcome($reading);
                if ($outcome === $ffi->SOUTHER_DECODED_VALUE) {
                    echo ", quantity ", field($ffi, "souther5_m_shop_t_Line_f_quantity",
                            $ffi->souther_decoded_value($reading), "int64_t")->cdata;
                } elseif ($outcome === $ffi->SOUTHER_DECODED_ISSUES) {
                    for ($at = 0; $at < $ffi->souther_decoded_issue_count($reading); $at++) {
                        $issue = $ffi->souther_decoded_issue($reading, $at);
                        echo ", [", text($ffi, $ffi->souther_issue_path($issue)), " ",
                                text($ffi, $ffi->souther_issue_code($issue)), "]";
                    }
                } else {
                    echo ", malformed at ", $ffi->souther_decoded_malformed_at($reading);
                }
                echo "\n";
            }

            $mark = $ffi->souther_mark();

            $three = $ffi->new("souther_value");
            $status = $ffi->souther5_m_shop_t_Money_construct(3, FFI::addr($three));
            echo "money: status $status, value ",
                    field($ffi, "souther5_m_shop_t_Money_f_value", $three, "int64_t")->cdata, "\n";
            $below = $ffi->new("souther_value");
            $status = $ffi->souther5_m_shop_t_Money_construct(-1, FFI::addr($below));
            echo "below: ", (int) ($status === $ffi->SOUTHER_INVARIANT_NOT_HELD && FFI::isNull($below)),
                    "\n";

            $wrap = "gift wrap";
            $note = $ffi->souther_string_of_utf8(bytes($ffi, $wrap), strlen($wrap));
            $line = $ffi->new("souther_value");
            $status = $ffi->souther5_m_shop_t_Line_construct($three, 2, 1, $note, FFI::addr($line));
            $noted = $ffi->new("souther_string");
            $noteThere = $ffi->new("uint8_t");
            $ffi->souther5_m_shop_t_Line_f_note($line, FFI::addr($noteThere), FFI::addr($noted));
            $present = $noteThere->cdata;
            echo "line: status $status, note $present ", text($ffi, $noted), "\n";

            $outcome = $ffi->new("souther_value");
            $status = $ffi->souther5_m_shop_b_settle(null, $line, 2, FFI::addr($outcome));
            $owed = $ffi->new("int64_t");
            $owing = $ffi->souther5_m_shop_b_owing(null, $outcome, FFI::addr($owed));
            echo "settled: status $status, case ", $ffi->souther5_m_shop_t_Outcome_case($outcome),
                    ", amount ", field($ffi, "souther5_m_shop_t_Money_f_value",
                            field($ffi, "souther5_m_shop_t_Owed_f_amount", $outcome, "souther_value"),
                            "int64_t")->cdata,
                    ", owing $owing ", $owed->cdata, "\n";

            $owes = $ffi->new("souther_value");
            $status = $ffi->souther5_m_shop_b_charge(null, 0, FFI::addr($owes));
            $free = $ffi->new("souther_value");
            $freed = $ffi->souther5_m_shop_b_charge(null, 1, FFI::addr($free));
            echo "charged: status $status, case ", $ffi->souther5_m_shop_b_charge_answer_case($owes),
                    ", status $freed, case ", $ffi->souther5_m_shop_b_charge_answer_case($free), "\n";

            $countedFive = $ffi->new("souther_value");
            $status = $ffi->souther5_m_shop_b_quantityOf(null, 5, FFI::addr($countedFive));
            $noneCounted = $ffi->new("souther_value");
            $nothing = $ffi->souther5_m_shop_b_quantityOf(null, 0, FFI::addr($noneCounted));
            $made = $ffi->souther_case_int_make(7);
            echo "counted: status $status, case ", $ffi->souther5_m_shop_b_quantityOf_answer_case($countedFive),
                    ", quantity ", $ffi->souther_case_int_read($countedFive), ", status $nothing, case ",
                    $ffi->souther5_m_shop_b_quantityOf_answer_case($noneCounted), ", made case ",
                    $ffi->souther5_m_shop_b_quantityOf_answer_case($made), " quantity ",
                    $ffi->souther_case_int_read($made), "\n";

            $both = $ffi->new("souther_value[2]");
            $both[0] = $line;
            $both[1] = $line;
            $lines = $ffi->souther5_m_shop_l_value_construct(2, $both);
            $second = $ffi->new("souther_value");
            $inside = $ffi->souther5_m_shop_l_value_at($lines, 1, FFI::addr($second));
            $past = $ffi->new("souther_value");
            $outside = $ffi->souther5_m_shop_l_value_at($lines, 2, FFI::addr($past));
            $before = $ffi->souther5_m_shop_l_value_at($lines, -1, FFI::addr($past));
            $counted = $ffi->new("int64_t");
            $status = $ffi->souther5_m_shop_b_counted(null, $lines, FFI::addr($counted));
            $doubled = $ffi->new("souther_list");
            $twice = $ffi->souther5_m_shop_b_doubled(null, $line, FFI::addr($doubled));
            echo "lines: length ", $ffi->souther5_m_shop_l_value_length($lines), ", at 1 $inside quantity ",
                    field($ffi, "souther5_m_shop_t_Line_f_quantity", $second, "int64_t")->cdata,
                    ", at 2 $outside, at -1 $before,",
                    " untouched ", (int) FFI::isNull($past), ", counted $status ", $counted->cdata,
                    ", doubled $twice ", $ffi->souther5_m_shop_l_value_length($doubled), "\n";

            $there = $ffi->new("uint8_t[2]");
            $there[0] = 0;
            $there[1] = 1;
            $said = $ffi->new("souther_string[2]");
            $said[0] = null;
            $said[1] = $note;
            $notes = $ffi->souther5_m_shop_l_o_string_construct(2, $there, $said);
            $ones = $ffi->new("int64_t[1]");
            $ones[0] = 1;
            $rows = $ffi->new("souther_list[2]");
            $rows[0] = $ffi->souther5_m_shop_l_int_construct(1, $ones);
            $rows[1] = $ffi->souther5_m_shop_l_int_construct(0, null);
            $groups = $ffi->souther5_m_shop_l_l_int_construct(2, $rows);
            $basket = $ffi->new("souther_value");
            $status = $ffi->souther5_m_shop_t_Basket_construct($lines, $notes, $groups, FFI::addr($basket));
            $firstThere = $ffi->new("uint8_t");
            $first = $ffi->new("souther_string");
            $ffi->souther5_m_shop_l_o_string_at(
                    field($ffi, "souther5_m_shop_t_Basket_f_notes", $basket, "souther_list"), 0,
                    FFI::addr($firstThere), FFI::addr($first));
            $secondThere = $ffi->new("uint8_t");
            $notedSecond = $ffi->new("souther_string");
            $ffi->souther5_m_shop_l_o_string_at(
                    field($ffi, "souther5_m_shop_t_Basket_f_notes", $basket, "souther_list"), 1,
                    FFI::addr($secondThere), FFI::addr($notedSecond));
            $group = $ffi->new("souther_list");
            $ffi->souther5_m_shop_l_l_int_at(
                    field($ffi, "souther5_m_shop_t_Basket_f_groups", $basket, "souther_list"), 0,
                    FFI::addr($group));
            $one = $ffi->new("int64_t");
            $ffi->souther5_m_shop_l_int_at($group, 0, FFI::addr($one));
            echo "basket: status $status, notes ", $firstThere->cdata, " ", (int) FFI::isNull($first), " ",
                    $secondThere->cdata, " ", text($ffi, $notedSecond), ", group ",
                    $ffi->souther5_m_shop_l_int_length($group), " ", $one->cdata, ", written ",
                    text($ffi, $ffi->souther5_m_shop_t_Basket_encode($basket)), "\n";

            $paired = $ffi->new("int64_t");
            $secondOf = $ffi->new("uint8_t");
            $status = $ffi->souther5_m_shop_v_pair(FFI::addr($paired), FFI::addr($secondOf));
            $outer = $ffi->new("uint8_t");
            $inner = $ffi->new("uint8_t");
            $held = $ffi->new("souther_string");
            $deepened = $ffi->souther5_m_shop_v_deep(FFI::addr($outer), FFI::addr($inner), FFI::addr($held));
            echo "pair: status $status, ", $paired->cdata, " ", $secondOf->cdata, ", deep: status $deepened, ",
                    $outer->cdata, " ", $inner->cdata, " ", (int) FFI::isNull($held), "\n";

            $pairs = $ffi->new("souther_list");
            $status = $ffi->souther5_m_shop_v_pairs(FFI::addr($pairs));
            $number = $ffi->new("int64_t");
            $letter = $ffi->new("souther_string");
            $ffi->souther5_m_shop_l_t2_int_string_at($pairs, 1, FFI::addr($number), FFI::addr($letter));
            $numbers = $ffi->new("int64_t[2]");
            $numbers[0] = 7;
            $numbers[1] = 8;
            $letters = $ffi->new("souther_string[2]");
            $letters[0] = $note;
            $letters[1] = $note;
            $built = $ffi->souther5_m_shop_l_t2_int_string_construct(2, $numbers, $letters);
            $eight = $ffi->new("int64_t");
            $notedEight = $ffi->new("souther_string");
            $ffi->souther5_m_shop_l_t2_int_string_at($built, 1, FFI::addr($eight), FFI::addr($notedEight));
            echo "pairs: status $status, length ", $ffi->souther5_m_shop_l_t2_int_string_length($pairs),
                    ", at 1 ", $number->cdata, " ", text($ffi, $letter), ", made ", $eight->cdata, " ",
                    text($ffi, $notedEight), "\n";

            echo "written: ", text($ffi, $ffi->souther5_m_shop_t_Line_encode($line)), "\n";
            decoded($ffi, "read", '{"price": 4, "quantity": 5}');
            decoded($ffi, "read wrong", '{"price": -1, "quantity": 5}');
            decoded($ffi, "not json", '{"price"');

            $ffi->souther_reset($mark);
            """;

    /** What both hosts are answered, which is the one program asked the same things. */
    private static final String ANSWERED = """
            money: status 0, value 3
            below: 1
            line: status 0, note 1 gift wrap
            settled: status 0, case 3, amount 4, owing 0 4
            charged: status 0, case 0, status 0, case 1
            counted: status 0, case 1, quantity 5, status 0, case 0, made case 1 quantity 7
            lines: length 2, at 1 1 quantity 2, at 2 0, at -1 0, untouched 1, counted 0 2, doubled 0 2
            basket: status 0, notes 0 1 1 gift wrap, group 1 1, written {"lines":[{"price":3,"quantity":2,"note":"gift wrap"},{"price":3,"quantity":2,"note":"gift wrap"}],"notes":[null,"gift wrap"],"groups":[[1],[]]}
            pair: status 0, 3 1, deep: status 0, 1 0 1
            pairs: status 0, length 2, at 1 2 b, made 8 gift wrap
            written: {"price":3,"quantity":2,"note":"gift wrap"}
            read: status 0, quantity 5
            read wrong: status 0, [/price invariant_violation]
            not json: status 0, malformed at 8
            """;

    /** What version 10 of the manifest is, for the program above. */
    private static final Path INTERFACE_V10 =
            Path.of("native", "crates", "compiler", "tests", "interface-v10.json");

    private static final JsonMapper JSON = JsonMapper.builder().build();

    @Test
    void aCProgramIncludingOnlyTheHeaderCallsTheLibrary(@TempDir Path into) throws Exception {
        assertThat(HARNESS).doesNotContain("__asm__").doesNotContain("extern");
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into);

        Path source = into.resolve("host.c");
        Files.writeString(source, HARNESS, StandardCharsets.UTF_8);
        Path executable = into.resolve("host");
        said(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), source.toString(),
                "-I", library.header().getParent().toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));

        assertThat(said(List.of(executable.toString()))).isEqualTo(ANSWERED);
    }

    @Test
    void phpDeclaresTheFunctionsFromTheDeclarationsAndCallsTheLibrary(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into);
        Path script = into.resolve("host.php");
        Files.writeString(script, PHP, StandardCharsets.UTF_8);

        assertThat(Php.ran(List.of("-d", "ffi.enable=1", script.toString(),
                library.declarations().toString(), library.library().toString())))
                .isEqualTo(ANSWERED);
    }

    /**
     * The manifest a binding is written against, as version 10 says it for this program. A change
     * to what the manifest says is a change here, and whether it moves the version is decided
     * looking at it.
     */
    @Test
    void theManifestIsWhatVersionTenSays(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into);

        String written = Files.readString(library.manifest(), StandardCharsets.UTF_8);
        String fixed = Files.exists(INTERFACE_V10)
                ? Files.readString(INTERFACE_V10, StandardCharsets.UTF_8) : "";
        if (!written.equals(fixed)) {
            // Kept where it can be compared with the fixture, and copied over it once it is read.
            Files.writeString(Path.of("target", "interface-v10.written.json"), written,
                    StandardCharsets.UTF_8);
        }
        assertThat(written).isEqualTo(fixed);
    }

    /**
     * The header, the manifest and the library name one set of functions, and the library exports
     * nothing a host does not call: not a row's entry, not a boundary, not what one object built by
     * this compiler reaches in another, and none of the runtime's own.
     */
    @Test
    void theHeaderTheManifestAndTheLibraryNameOneSet(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into);

        String declarations = Files.readString(library.declarations());
        assertThat(declarations.lines()).noneMatch(line -> line.strip().startsWith("#"));
        assertThat(Files.readString(library.header()))
                .contains("#include \"" + library.declarations().getFileName() + "\"");
        Set<String> declared = declaredIn(declarations);
        Set<String> described = describedIn(JSON.readTree(library.manifest().toFile()));
        Set<String> exported = exportedBy(library.library());

        assertThat(declared).isNotEmpty();
        assertThat(described).isEqualTo(declared);
        assertThat(exported).isEqualTo(declared);

        // So that what is not exported is something the object does hold.
        Set<String> inTheObject = definedIn(library.object());
        assertThat(inTheObject).anyMatch(it -> it.contains("$example$"));
        assertThat(inTheObject).anyMatch(it -> it.endsWith("$boundary"));
        // What a host implements has no symbol of its own: it is reached only through the
        // capability a host makes of an implementation, which the object makes.
        assertThat(inTheObject).contains("souther" + Running.ABI + ".shop.settle",
                "souther" + Running.ABI + "_m_shop_b_discountFor_implement");
        assertThat(inTheObject).doesNotContain("souther" + Running.ABI + ".shop.discountFor");
        assertThat(exported).noneMatch(it -> it.contains("$") || it.contains("."));
        assertThat(exported).doesNotContain("souther_alloc", "souther_decode_begin",
                "souther_read_int", "souther_external_json");
    }

    private static final Pattern DECLARATION = Pattern.compile("([A-Za-z0-9_]+)\\(.*\\);$");

    private static Set<String> declaredIn(String header) {
        Set<String> declared = new TreeSet<>();
        for (String line : header.lines().toList()) {
            Matcher found = DECLARATION.matcher(line);
            if (found.find()) {
                declared.add(found.group(1));
            }
        }
        return declared;
    }

    /**
     * Every function the manifest names, wherever it names one: every member shaped as a function
     * is (a name, what it takes and what it answers, and nothing else), and what a host makes a
     * capability of an implementation through, and not what it implements, whose name is a type's. Found by
     * walking the whole manifest rather than by a list of where functions are kept, so a function
     * a later version puts somewhere new is held to the header and the library without this
     * having to be told.
     */
    private static Set<String> describedIn(JsonNode manifest) {
        Set<String> described = new TreeSet<>();
        walk(manifest, described);
        return described;
    }

    private static void walk(JsonNode node, Set<String> described) {
        if (node.isObject()) {
            if (new TreeSet<>(node.propertyNames()).equals(Set.of("answers", "name", "takes"))) {
                described.add(node.get("name").stringValue());
            }
            if (node.has("implement")) {
                described.add(node.get("implement").stringValue());
            }
        }
        node.forEach(it -> walk(it, described));
    }

    private static Set<String> exportedBy(Path library) throws IOException, InterruptedException {
        return symbols(switch (Running.HOST) {
            case DARWIN -> List.of("nm", "-gU", library.toString());
            case LINUX -> List.of("nm", "-D", "--defined-only", library.toString());
        });
    }

    private static Set<String> definedIn(Path object) throws IOException, InterruptedException {
        return symbols(switch (Running.HOST) {
            case DARWIN -> List.of("nm", "-gU", object.toString());
            case LINUX -> List.of("nm", "-g", "--defined-only", object.toString());
        });
    }

    private static Set<String> symbols(List<String> command)
            throws IOException, InterruptedException {
        Set<String> named = new TreeSet<>();
        for (String line : said(command).lines().toList()) {
            String[] fields = line.trim().split("\\s+");
            if (fields.length == 3) {
                String name = fields[2];
                named.add(name.startsWith(Running.PREFIX)
                        ? name.substring(Running.PREFIX.length()) : name);
            }
        }
        return named;
    }

    private static String said(List<String> command) throws IOException, InterruptedException {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError(command.get(0) + " failed: " + said);
        }
        return said;
    }
}
