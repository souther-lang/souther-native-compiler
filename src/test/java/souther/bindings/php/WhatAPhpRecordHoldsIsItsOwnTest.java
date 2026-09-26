package souther.bindings.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.CrossingShape;
import souther.bindings.Manifest.Case;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.Word;
import souther.bindings.Owning;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every record the PHP generator holds a collection in owns it, as the shared package's records do
 * ({@code WhatARecordHoldsIsItsOwnTest}), and every such record the package declares is asked of.
 * What a generation answers is asked of as a caller holds it; what only the generator makes is made
 * here as it makes it.
 */
class WhatAPhpRecordHoldsIsItsOwnTest {

    @Test
    void everyCollectionAPhpRecordHoldsIsItsOwn(@TempDir Path into) throws Exception {
        PhpBindings.Generated generated = LibraryBinding.generated(NativeCompiler.library(
                CheckedProgram.of(List.of("""
                        module m exposing ( Kept )

                        data Kept = Int
                        """)), into.resolve("native")), into.resolve("php"), "Acme");
        Type.Union either = new Type.Union(List.of(
                new Case.Declared("m", "Found"), new Case.Declared("m", "Missing")));
        Crossing.Member found = new Crossing.Member(Crossing.Whole.product(
                CrossingShape.declared("m", "Found"), "\\Acme\\M\\Found"), null);
        Crossing.Member missing = new Crossing.Member(Crossing.Whole.product(
                CrossingShape.declared("m", "Missing"), "\\Acme\\M\\Missing"), null);
        Function which = new Function("which", List.of(Parameter.given(Word.VALUE)), Word.CASE);

        Owning owning = new Owning();
        owning.walk(generated);
        owning.walk(new Crossing.OneOf(new CrossingShape.Whole(Word.VALUE, either),
                List.of(found, missing)));
        owning.walk(new Crossing.Told(new CrossingShape.Told(either, either.cases(), which),
                "\\Acme\\M\\Found|\\Acme\\M\\Missing", List.of(found, missing), "`m.find`"));

        assertThat(owning.wrong).isEmpty();
        assertThat(Owning.collectionsIn("souther.bindings.php")).contains(
                PhpBindings.Generated.class.getName() + ".files",
                Crossing.OneOf.class.getName() + ".members");
        assertThat(owning.asked).containsAll(Owning.collectionsIn("souther.bindings.php"));
    }
}
