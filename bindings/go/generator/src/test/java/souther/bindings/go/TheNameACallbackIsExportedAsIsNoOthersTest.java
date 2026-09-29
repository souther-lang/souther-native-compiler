package souther.bindings.go;

import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.io.ByteArrayOutputStream;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a callback is exported as is a name of the whole program, so it is made of what says which
 * callback it is in a way that is read back as those parts and no others. Held here as a property
 * and not for the pairs that once came to one: what Go does not take in a name is replaced in a way
 * that two paths, two modules or two names could all share.
 */
class TheNameACallbackIsExportedAsIsNoOthersTest {

    private record Parts(char kind, String importPath, String module, String name) {
    }

    private static String symbol(Parts it) {
        return GoNames.hostSymbol(it.kind(), it.importPath(), it.module(), it.name());
    }

    /** The parts a symbol was made of, read only from the symbol. */
    private static Parts read(String symbol) {
        String prefix = "souther_host_z";
        assertThat(symbol).startsWith(prefix);
        String rest = symbol.substring(prefix.length());
        char kind = rest.charAt(0);
        List<String> parts = new java.util.ArrayList<>();
        StringBuilder each = new StringBuilder();
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        int at = 1;
        // "_z" begins each part.
        assertThat(rest.startsWith("_z", at)).isTrue();
        at += 2;
        while (at <= rest.length()) {
            if (at == rest.length() || rest.startsWith("_z", at)) {
                parts.add(new String(bytes.toByteArray(), StandardCharsets.UTF_8));
                bytes.reset();
                at += 2;
                continue;
            }
            char it = rest.charAt(at);
            if (it != '_') {
                bytes.write(it);
                at++;
            } else if (rest.charAt(at + 1) == '_') {
                bytes.write('_');
                at += 2;
            } else {
                assertThat(rest.charAt(at + 1)).isEqualTo('u');
                bytes.write(Integer.parseInt(rest.substring(at + 2, at + 4), 16));
                assertThat(rest.charAt(at + 4)).isEqualTo('_');
                at += 5;
            }
        }
        assertThat(parts).hasSize(3);
        assertThat(each).isEmpty();
        return new Parts(kind, parts.get(0), parts.get(1), parts.get(2));
    }

    @Test
    void thePairsThatOnceCameToOneAreTwo() {
        assertThat(symbol(new Parts('i', "example.com/a-b", "m", "pick")))
                .isNotEqualTo(symbol(new Parts('i', "example.com/a_b", "m", "pick")));
        assertThat(symbol(new Parts('i', "p", "a.x", "pick")))
                .isNotEqualTo(symbol(new Parts('i', "p", "b.x", "pick")));
        assertThat(symbol(new Parts('i', "p", "m", "x")))
                .isNotEqualTo(symbol(new Parts('f', "p", "m", "x")));
        // A name that holds what separates the parts, or what escapes a byte.
        assertThat(symbol(new Parts('i', "p", "a", "z_x")))
                .isNotEqualTo(symbol(new Parts('i', "p", "a_z", "x")));
        assertThat(symbol(new Parts('i', "p", "a", "b")))
                .isNotEqualTo(symbol(new Parts('i', "p", "a_zb", "")));
        assertThat(symbol(new Parts('i', "p", "_u2e_", "x")))
                .isNotEqualTo(symbol(new Parts('i', "p", ".", "x")));
    }

    @Test
    void everySymbolIsReadBackAsTheOnePartsItWasMadeOfAndIsAName() {
        String alphabet = "ab_zu-./é0Z";
        Random random = new Random(84);
        Map<String, Parts> seen = new HashMap<>();
        for (int round = 0; round < 40_000; round++) {
            Parts it = new Parts(random.nextBoolean() ? 'i' : 'f', text(random, alphabet),
                    text(random, alphabet), text(random, alphabet));
            String symbol = symbol(it);

            assertThat(symbol).matches("[A-Za-z0-9_]+");
            assertThat(read(symbol)).isEqualTo(it);
            Parts before = seen.putIfAbsent(symbol, it);
            assertThat(before == null || before.equals(it))
                    .as("%s and %s are both %s", before, it, symbol).isTrue();
        }
    }

    private static String text(Random random, String alphabet) {
        StringBuilder out = new StringBuilder();
        for (int at = random.nextInt(5); at > 0; at--) {
            out.append(alphabet.charAt(random.nextInt(alphabet.length())));
        }
        return out.toString();
    }
}
