package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.Manifest;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every row of a behavior a host implements is on the manifest, as what a host does with it, and
 * the rows of a behavior with a body are not: those are this project's own check of a native run,
 * and a host is told nothing of them.
 */
class TheRowsOfABehaviorAHostImplementsAreListedTest {

    private static final String LISTED = """
            module listed exposing ( Missing, found, counted, twice )

            data Missing

            behavior found : (n: Int) -> Int | Missing

            behavior counted : (n: Int) -> Int

            behavior twice : (n: Int) -> Int
            let twice (n) = n * 2

            example found
                | "a number where a union is answered" : (1) -> 1
                | "missing" : (2) -> Missing

            example counted
                | "three" : (1) -> 3
                | (2) -> <?>

            example twice
                | "doubled" : (1) -> 2
            """;

    @Test
    void eachRowIsListedAsWhatAHostDoesWithIt(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(Checked.of(List.of(LISTED)), into);
        Manifest.Module module = ManifestReader.read(library.manifest()).modules().getFirst();

        assertThat(module.injections()).extracting(Manifest.Injection::name)
                .containsExactly("found", "counted");
        assertThat(rows(module, "found")).containsExactly(
                "0 a number where a union is answered: asserted",
                "1 missing: asserted");
        assertThat(rows(module, "counted")).containsExactly(
                "0 three: asserted",
                "1 row 2: owed");
    }

    private static List<String> rows(Manifest.Module module, String behavior) {
        Manifest.Injection injection = module.injections().stream()
                .filter(it -> it.name().equals(behavior))
                .findFirst().orElseThrow();
        return injection.examples().stream()
                .map(it -> it.at() + " " + it.identity().shown() + ": " + switch (it.row()) {
                    case Manifest.Row.Asserted(Manifest.Reach.Available<?> run) -> "asserted";
                    case Manifest.Row.Asserted(Manifest.Reach.Unavailable<?> nothing) ->
                            "not compared";
                    case Manifest.Row.Owed owed -> "owed";
                    case Manifest.Row.Unreproducible(String why) -> "not reproducible: " + why;
                })
                .toList();
    }
}
