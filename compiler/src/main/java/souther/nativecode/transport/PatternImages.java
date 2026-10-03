package souther.nativecode.transport;

import java.util.List;
import net.unit8.notation199x.pattern.PatternImage;
import net.unit8.notation199x.pattern.PatternMachine;
import souther.compiler.regex.CodePoints;
import souther.compiler.regex.PatternMeaning;
import souther.nativecode.NotLowered;

/**
 * The image a pattern crosses to the native side as: the machine of what the checker read it as,
 * written by 199x-notation in a format the runtime reads with the same repository's Rust crate.
 *
 * <p>What a pattern means is the checker's, and nothing here reads pattern text: the parts the
 * checker read are put as they are into 199x-notation's own meaning, which has the same six, and
 * the machine is built and written from that. So the native side neither builds a machine nor
 * says what a word of one is, and what matches is what the formats define (199x-notation's
 * {@code image/P1.md}).
 */
final class PatternImages {

    private PatternImages() {}

    /**
     * The image of {@code meaning}, which the checker read {@code written} as.
     *
     * @throws NotLowered where the machine has more states than 199x-notation builds one of, or
     *                    its image more characters than one image is given: a limit of carrying a
     *                    machine to where it runs, and not of what the pattern means
     */
    static String of(String written, PatternMeaning meaning) {
        PatternMachine machine;
        try {
            machine = PatternMachine.of(translated(meaning));
        } catch (IllegalArgumentException tooMany) {
            throw new NotLowered("the pattern " + written + " has no machine this backend can"
                    + " carry: " + tooMany.getMessage());
        }
        return switch (machine.image()) {
            case PatternImage.Written it -> String.join("", it.strings());
            case PatternImage.MoreCharacters it -> throw new NotLowered("the pattern " + written
                    + " has a machine that takes more than " + it.most()
                    + " characters to write as an image");
        };
    }

    /** {@code meaning} as 199x-notation's meaning, part for part. */
    private static net.unit8.notation199x.pattern.PatternMeaning translated(PatternMeaning meaning) {
        return switch (meaning) {
            case PatternMeaning.Nothing it -> new net.unit8.notation199x.pattern.PatternMeaning.Nothing();
            case PatternMeaning.Never it -> new net.unit8.notation199x.pattern.PatternMeaning.Never();
            case PatternMeaning.Symbols it -> new net.unit8.notation199x.pattern.PatternMeaning.Symbols(
                    new net.unit8.notation199x.pattern.CodePoints(it.held().ranges().stream()
                            .map(PatternImages::translated)
                            .toList()));
            case PatternMeaning.InTurn it -> new net.unit8.notation199x.pattern.PatternMeaning.InTurn(
                    translated(it.parts()));
            case PatternMeaning.EitherOf it -> new net.unit8.notation199x.pattern.PatternMeaning.EitherOf(
                    translated(it.arms()));
            case PatternMeaning.Repeated it -> new net.unit8.notation199x.pattern.PatternMeaning.Repeated(
                    translated(it.what()), it.least(),
                    it.unbounded()
                            ? net.unit8.notation199x.pattern.PatternMeaning.Repeated.NO_CEILING
                            : it.most());
        };
    }

    private static List<net.unit8.notation199x.pattern.PatternMeaning> translated(
            List<PatternMeaning> each) {
        return each.stream().map(PatternImages::translated).toList();
    }

    private static net.unit8.notation199x.pattern.CodePoints.Range translated(CodePoints.Range range) {
        return new net.unit8.notation199x.pattern.CodePoints.Range(range.from(), range.to());
    }
}
