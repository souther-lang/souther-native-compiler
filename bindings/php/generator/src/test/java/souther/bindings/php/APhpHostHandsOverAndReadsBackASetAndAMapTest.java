package souther.bindings.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import souther.nativecode.Repository;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A PHP host hands the library a set as a PHP list of its members, and a map as a PHP list of its
 * entries, each a list of its key and its value, and is handed each back the same way: what a
 * behavior takes and answers, a field it builds a value with and reads back, and what a behavior
 * the host implements answers.
 *
 * <p>The library makes a set of what it is handed, so two members that are one are one, and a map
 * of the entries, the later of two under one key winning. The order a set or a map is handed back
 * in is no order the language says anything of, so the host sorts what it prints; what the value
 * is written as at a boundary is in the order the language fixes.
 */
class APhpHostHandsOverAndReadsBackASetAndAMapTest {

    private static final String TALLY = """
            module tally exposing ( Tally, sized, counted, total, tallied, asked )

            data Tally = { tags: Set<String>, counts: Map<String, Int> }

            behavior sized : (s: Set<Int>) -> Int
            let sized (s) = Set.size(s)

            behavior counted : (words: List<String>) -> Map<String, Int>
            let counted (words) =
                List.fold((acc, w) -> Map.updateOrInsert(w, 1, n -> n + 1, acc), Map.empty, words)

            behavior total : (m: Map<String, Int>) -> Int
            let total (m) = Map.fold((acc, k, v) -> acc + v, 0, m)

            behavior tallied : (t: Tally) -> Int
            let tallied (t) = Set.size(t.tags) * 100 + Map.size(t.counts)

            behavior drawn : (n: Int) -> Set<Int>

            behavior asked : (n: Int) -> Int
                depends on drawn
            let asked (n, drawn) = Set.size(drawn(n))
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Billing\\Binding;
            use Acme\\Billing\\Tally\\Behaviors;
            use Acme\\Billing\\Tally\\Injections;
            use Acme\\Billing\\Tally\\Tally;

            /** @param list<string|int> $members */
            function sorted(array $members): string {
                sort($members);
                return implode(',', $members);
            }

            /** @param list<array{0: string, 1: int}> $entries */
            function entries(array $entries): string {
                usort($entries, fn ($a, $b) => strcmp($a[0], $b[0]));
                return implode(',', array_map(fn ($it) => $it[0] . '=' . $it[1], $entries));
            }

            $binding = Binding::load($argv[3]);

            $binding->run(function (): void {
                echo "sized: ", Behaviors::sized([3, 1, 3, 2]), "\\n";
                echo "counted: ", entries(Behaviors::counted(['b', 'a', 'b'])), "\\n";
                echo "total: ", Behaviors::total([['a', 1], ['b', 2], ['a', 5]]), "\\n";
                $tally = Tally::of(['y', 'x', 'y'], [['k', 1], ['j', 2]])->getOrThrow();
                echo "tags: ", sorted($tally->tags()), "\\n";
                echo "counts: ", entries($tally->counts()), "\\n";
                echo "tallied: ", Behaviors::tallied($tally), "\\n";
                echo "written: ", $tally->encode(), "\\n";
                echo "read: ", sorted(Tally::decode($tally->encode())->getOrThrow()->tags()), "\\n";
            });

            $drawing = Injections::of(drawn: fn (int $n): array => [$n, 1, $n, 2]);
            echo "asked: ", $binding->run(fn (): int => Behaviors::asked(5), $drawing), "\\n";
            """;

    private static final String ANSWERED = """
            sized: 3
            counted: a=1,b=2
            total: 7
            tags: x,y
            counts: j=2,k=1
            tallied: 202
            written: {"tags":["x","y"],"counts":{"j":2,"k":1}}
            read: x,y
            asked: 3
            """;

    /** Where the runtime package stands, with what Composer installed for it. */
    private static final Path RUNTIME = Repository.file("bindings", "php", "runtime");

    @Test
    void aSetAndAMapCrossAsListsOfWhatTheyHold(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(TALLY)), into.resolve("native"));
        Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Billing");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()));

        assertThat(said).isEqualTo(ANSWERED);
    }
}
