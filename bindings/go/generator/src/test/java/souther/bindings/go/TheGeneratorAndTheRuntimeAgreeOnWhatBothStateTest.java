package souther.bindings.go;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What the generator writes for and what the runtime module offers are two files, and what both
 * state is stated once in each: the protocol a package is written for, and how many members a tuple
 * has. Held here, so that one moving without the other is a failure of this and not a package that
 * does not build, or a tuple that is not written where the runtime has its type.
 */
class TheGeneratorAndTheRuntimeAgreeOnWhatBothStateTest {

    private static String runtime(String file) throws IOException {
        return Files.readString(GoHost.RUNTIME.resolve(file), StandardCharsets.UTF_8);
    }

    @Test
    void thePackagesAreWrittenForTheProtocolTheRuntimeIs() throws IOException {
        Matcher protocol = Pattern.compile("const Protocol = (\\d+)").matcher(runtime("protocol.go"));

        assertThat(protocol.find()).isTrue();
        assertThat(GoBindings.RUNTIME_PROTOCOL).isEqualTo(Integer.parseInt(protocol.group(1)));
    }

    @Test
    void aTupleIsWrittenForEverySizeTheRuntimeHasATypeOfAndNoOther() throws IOException {
        Matcher types = Pattern.compile("type Tuple(\\d+)\\[").matcher(runtime("tuple.go"));
        List<Integer> sizes = new ArrayList<>();
        while (types.find()) {
            sizes.add(Integer.parseInt(types.group(1)));
        }

        assertThat(sizes).containsExactlyElementsOf(java.util.stream.IntStream
                .rangeClosed(2, Crossing.Tuple.MOST).boxed().toList());
    }
}
