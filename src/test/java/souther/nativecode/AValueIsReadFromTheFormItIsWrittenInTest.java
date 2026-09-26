package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A host reads a value of a published type out of JSON through the decoder the object defines for
 * it, and writes one back through the encoder, and is told what was wrong with a document as
 * Raoh's issues.
 *
 * <p>What a document is read as is the language's, as what a value is written as is: the checker
 * settles every position's shape and the program carries it. So the rows here are the language's
 * rules read off a run — a field left out is absent where it may be and missing where it may not,
 * a clause that does not hold is an issue where the value is, every mistake in a document is
 * answered and not the first — and the one thing a host decides is what to do with the answer.
 */
class AValueIsReadFromTheFormItIsWrittenInTest {

    private static final String WIRE = """
            module wire exposing ( Money, Line, Chain, Won, Lost, Stage, Manager, Staff, Rank,
                                   Free, Paid, Settled, Ratio, Tag )

            data Money = Int
                invariant notNegative = value >= 0

            data Line = { price: Money, quantity: Int, note: String?, gift: Bool? }
                invariant quantity > 0

            data Chain = { n: Int, next: Chain? }

            data Won
            data Lost
            data Stage = Won | Lost

            data Manager = Int
            data Staff
            data Rank = Manager | Staff

            data Free
            data Paid = { amount: Money, stage: Stage }
            data Settled = Free | Paid

            data Ratio = { top: Int, bottom: Int }
                invariant fits = top * bottom >= 0

            data Tag = String
            """;

    private static final Decoding ROWS = new Decoding()
            .type("wire", "Money").type("wire", "Line").type("wire", "Chain")
            .type("wire", "Stage").type("wire", "Rank").type("wire", "Settled")
            .type("wire", "Free").type("wire", "Ratio").type("wire", "Tag")
            .row("line", "Line", "{\"price\":3,\"quantity\":2,\"note\":\"gift wrap\",\"gift\":true}")
            .row("line absent", "Line", "{\"price\":3,\"quantity\":2}")
            .row("line null", "Line", "{\"price\":3,\"quantity\":2,\"note\":null,\"gift\":null}")
            .row("line wrong", "Line", "{\"price\":-1,\"quantity\":\"two\",\"gift\":1}")
            .row("line empty", "Line", "{}")
            .row("line array", "Line", "[]")
            .row("line none", "Line", "{\"price\":3,\"quantity\":0}")
            .row("line nfc", "Line", "{\"price\":1,\"quantity\":1,\"note\":\"か\\u3099\"}")
            .row("line cut", "Line", "{\"price\":3,")
            .row("line and more", "Line", "{\"price\":3,\"quantity\":1}x")
            .row("line twice", "Line", "{\"price\":3,\"price\":-5,\"quantity\":1}")
            .row("line more", "Line", "{\"price\":3,\"quantity\":1,\"colour\":\"red\"}")
            .row("chain", "Chain", "{\"n\":1,\"next\":{\"n\":2}}")
            .row("chain deep", "Chain", "{\"n\":1,\"next\":{\"n\":2,\"next\":{\"n\":\"x\"}}}")
            .row("stage", "Stage", "\"Won\"")
            .row("stage unknown", "Stage", "\"Draw\"")
            .row("stage number", "Stage", "3")
            .row("rank manager", "Rank", "{\"type\":\"Manager\",\"value\":3}")
            .row("rank staff", "Rank", "{\"type\":\"Staff\"}")
            .row("rank empty", "Rank", "{\"type\":\"Manager\"}")
            .row("rank untagged", "Rank", "{\"value\":3}")
            .row("rank unknown", "Rank", "{\"type\":\"Boss\"}")
            .row("rank numbered", "Rank", "{\"type\":1}")
            .row("settled free", "Settled", "{\"type\":\"Free\"}")
            .row("settled paid", "Settled", "{\"type\":\"Paid\",\"amount\":6,\"stage\":\"Won\"}")
            .row("settled wrong", "Settled", "{\"type\":\"Paid\",\"amount\":-2,\"stage\":\"Tie\"}")
            .row("free", "Free", "{}")
            .row("free null", "Free", "null")
            .row("money", "Money", "5")
            .row("money below", "Money", "-5")
            .row("money amount", "Money", "5.0")
            .row("money past", "Money", "9223372036854775808")
            .row("ratio", "Ratio", "{\"top\":2,\"bottom\":3}")
            .row("ratio past", "Ratio", "{\"top\":4611686018427387904,\"bottom\":4}")
            .row("tag", "Tag", "\"x\\\"y\"");

    @Test
    void aDocumentIsReadAsTheLanguageWritesAValueAndEveryMistakeInItIsAnswered() throws Exception {
        String said = run(Checked.of(List.of(WIRE)), ROWS.harness());

        assertThat(said).isEqualTo("""
                line: value {"price":3,"quantity":2,"note":"gift wrap","gift":true}
                line absent: value {"price":3,"quantity":2}
                line null: value {"price":3,"quantity":2}
                line wrong: issues [@/price invariant_violation module=wire type=Money clause=notNegative] [@/quantity type_mismatch actual=string expected=Int] [@/gift type_mismatch actual=number expected=Bool]
                line empty: issues [@/price missing_field actual=nothing expected=a field] [@/quantity missing_field actual=nothing expected=a field]
                line array: issues [@ type_mismatch actual=array expected=an object]
                line none: issues [@ invariant_violation module=wire type=Line]
                line nfc: value {"price":1,"quantity":1,"note":"が"}
                line cut: malformed at 11
                line and more: malformed at 24
                line twice: value {"price":3,"quantity":1}
                line more: value {"price":3,"quantity":1}
                chain: value {"n":1,"next":{"n":2}}
                chain deep: issues [@/next/next/n type_mismatch actual=string expected=Int]
                stage: value "Won"
                stage unknown: issues [@ not_allowed actual=Draw expected=a case]
                stage number: issues [@ type_mismatch actual=number expected=a case]
                rank manager: value {"type":"Manager","value":3}
                rank staff: value {"type":"Staff"}
                rank empty: issues [@/value missing_field actual=nothing expected=a field]
                rank untagged: issues [@/type missing_field actual=nothing expected=a case]
                rank unknown: issues [@/type not_allowed actual=Boss expected=a case]
                rank numbered: issues [@/type type_mismatch actual=number expected=a case]
                settled free: value {"type":"Free"}
                settled paid: value {"type":"Paid","amount":6,"stage":"Won"}
                settled wrong: issues [@/amount invariant_violation module=wire type=Money clause=notNegative] [@/stage not_allowed actual=Tie expected=a case]
                free: value {}
                free null: issues [@ type_mismatch actual=null expected=an object]
                money: value 5
                money below: issues [@ invariant_violation module=wire type=Money clause=notNegative]
                money amount: issues [@ type_mismatch actual=number expected=Int]
                money past: issues [@ out_of_range actual=number expected=Int]
                ratio: value {"top":2,"bottom":3}
                ratio past: status %d
                tag: value "x\\"y"
                """.formatted(overflow()));
    }

    /**
     * A value read and a value a host built with the type's constructor are one kind of value:
     * each is written back the same way, and each is read by the same readers.
     */
    @Test
    void aValueReadIsTheValueTheConstructorBuilds() throws Exception {
        String built = """
                    extern uint32_t money(int64_t, Value *) __asm__("%1$s");
                    extern uint32_t line(Value, int64_t, uint8_t, Value, uint8_t, uint8_t, Value *)
                            __asm__("%2$s");
                    extern Value souther_string_of_utf8(const uint8_t *, int64_t);
                    extern void lineQuantity(Value, int64_t *) __asm__("%3$s");
                    {
                        int64_t mark = souther_mark();
                        Value three = 0;
                        money(3, &three);
                        Value note = souther_string_of_utf8((const uint8_t *) "gift wrap", 9);
                        Value bought = 0;
                        line(three, 2, 1, note, 1, 1, &bought);
                        printf("built: ");
                        text(encodeLine(bought));
                        printf("\\n");

                        const char *document = "{\\"price\\":3,\\"quantity\\":2,\\"note\\":\\"gift wrap\\",\\"gift\\":true}";
                        Value read = 0;
                        decodeLine((const uint8_t *) document, (int64_t) strlen(document), &read);
                        int64_t quantity = -1;
                        lineQuantity(souther_decoded_value(read), &quantity);
                        printf("read quantity: %%lld\\n", (long long) quantity);
                        souther_reset(mark);
                    }
                """.formatted(
                Decoding.symbol("wire", "Money", "construct"),
                Decoding.symbol("wire", "Line", "construct"),
                Decoding.symbol("wire", "Line", "field$quantity"));
        String harness = new Decoding().type("wire", "Line").first(built)
                .row("read", "Line", "{\"price\":3,\"quantity\":2,\"note\":\"gift wrap\",\"gift\":true}")
                .harness();

        assertThat(run(Checked.of(List.of(WIRE)), harness)).isEqualTo("""
                built: {"price":3,"quantity":2,"note":"gift wrap","gift":true}
                read quantity: 2
                read: value {"price":3,"quantity":2,"note":"gift wrap","gift":true}
                """);
    }

    /**
     * A decoder and an encoder for every published type with a form this backend reads and writes,
     * and a reader of each type built from fields under the name another build reaches it by. A
     * type holding something with no form here has neither.
     */
    @Test
    void whatAHostReadsAndWritesIsWhatTheModulePublishesWithAForm() throws Exception {
        Set<String> defined = NativeArtifacts.built(Checked.of(List.of("""
                module shop exposing ( Money, Partial, Kind )

                data Money = Int
                data Partial = { count: Int, amount: Decimal }
                data Big
                data Small
                data Kind = Big | Small
                """))).defined();

        assertThat(defined).contains(
                Decoding.symbol("shop", "Money", "decode"), Decoding.symbol("shop", "Money", "encode"),
                Decoding.symbol("shop", "Kind", "decode"), Decoding.symbol("shop", "Kind", "encode"),
                Running.PREFIX + "souther" + Running.ABI + ".shop$read$Money");
        assertThat(defined).doesNotContain(
                Decoding.symbol("shop", "Partial", "decode"),
                Decoding.symbol("shop", "Partial", "encode"),
                Running.PREFIX + "souther" + Running.ABI + ".shop$read$Partial");
    }

    /**
     * The keys a set of alternatives travels under are the ones the program carries, read the way
     * they are written. The language writes {@code type} and {@code value} today and nothing else,
     * so the document is given two others by hand: a reader that had either spelled in would read
     * nothing here.
     */
    @Test
    void theKeysASetOfAlternativesTravelsUnderAreTheOnesTheProgramCarries() throws Exception {
        String written = ProgramWriter.written(Checked.of(List.of(WIRE)));
        String discriminated = "\"tag\":\"type\",\"contents\":\"value\"";
        assertThat(written).contains(discriminated);
        byte[] object = NativeCompiler.driven(written.replace(discriminated,
                "\"tag\":\"kind\",\"contents\":\"body\""));

        String harness = new Decoding().type("wire", "Rank")
                .row("manager", "Rank", "{\"kind\":\"Manager\",\"body\":3}")
                .row("staff", "Rank", "{\"kind\":\"Staff\"}")
                .row("spelt as today", "Rank", "{\"type\":\"Manager\",\"value\":3}")
                .row("empty", "Rank", "{\"kind\":\"Manager\",\"value\":3}")
                .harness();
        Process process = new ProcessBuilder(NativeArtifacts.linked(object, harness).toString())
                .redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        assertThat(process.waitFor()).as(said).isZero();

        assertThat(said).isEqualTo("""
                manager: value {"kind":"Manager","body":3}
                staff: value {"kind":"Staff"}
                spelt as today: issues [@/kind missing_field actual=nothing expected=a case]
                empty: issues [@/body missing_field actual=nothing expected=a field]
                """);
    }

    /**
     * What a clause leaving an `Int`'s range ends a run with, as this backend numbers it: the
     * language's reason for an `Int` that has no place for what it would hold.
     */
    private static int overflow() {
        for (int status = 1; ; status++) {
            if (Running.abortKindOf(status) == AbortKind.REQUIRED_FORM_HAS_NO_PLACE) {
                return status;
            }
        }
    }

    static String run(CheckedProgram program, String harness) throws Exception {
        return run(program, List.of(), harness);
    }

    static String run(CheckedProgram program, List<NativeArtifacts.Bytes> alongside, String harness)
            throws Exception {
        Path run = NativeArtifacts.executable(program, alongside, harness);
        Process process = new ProcessBuilder(run.toString()).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        assertThat(process.waitFor()).as(said).isZero();
        return said;
    }
}
