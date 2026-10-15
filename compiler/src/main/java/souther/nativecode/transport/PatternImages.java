package souther.nativecode.transport;

import net.unit8.notation199x.pattern.PatternImage;
import net.unit8.notation199x.pattern.PatternMachine;
import net.unit8.notation199x.pattern.PatternMeaning;
import souther.nativecode.NotLowered;

/**
 * The image a pattern crosses to the native side as: the machine of what the checker read it as,
 * written by notation-199x in a format the runtime reads with the same repository's Rust crate.
 *
 * <p>What a pattern means is the checker's, and nothing here reads pattern text: the checker reads
 * a pattern into notation-199x's own meaning, and the machine is built and written from that as it
 * is. So the native side neither builds a machine nor says what a word of one is, and what matches
 * is what the formats define (notation-199x's {@code image/P1.md}).
 */
final class PatternImages {

    private PatternImages() {}

    /**
     * The image of {@code meaning}, which the checker read {@code written} as.
     *
     * @throws NotLowered where the machine has more states than notation-199x builds one of, or
     *                    its image more characters than one image is given: a limit of carrying a
     *                    machine to where it runs, and not of what the pattern means
     */
    static String of(String written, PatternMeaning meaning) {
        PatternMachine machine;
        try {
            machine = PatternMachine.of(meaning);
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
}
