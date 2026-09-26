package souther.bindings.php;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.Manifest;
import souther.bindings.Manifest.Case;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.Word;
import souther.bindings.Owning;
import souther.nativecode.Documents;
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
                Checked.of(List.of("""
                        module m exposing ( Kept )

                        data Kept = Int
                        """)), into.resolve("native")), into.resolve("php"), "Acme");
        Type.Union either = new Type.Union(List.of(
                new Case.Declared("m", "Found"), new Case.Declared("m", "Missing")));
        Crossing.Member found = new Crossing.Member(Crossing.Whole.product("\\Acme\\M\\Found"),
                null);
        Crossing.Member missing = new Crossing.Member(
                Crossing.Whole.product("\\Acme\\M\\Missing"), null);
        Crossing.Whole count = java.util.Objects.requireNonNull(Crossing.Whole.primitive(Word.INT));
        Function which = new Function("which", List.of(Parameter.given(Word.VALUE)), Word.CASE);

        Owning owning = new Owning();
        owning.walk(generated);
        owning.walk(new Crossing.OneOf(either, List.of(found, missing)));
        owning.walk(new Crossing.Told(which, "\\Acme\\M\\Found|\\Acme\\M\\Missing",
                List.of(found, missing), "`m.find`"));
        owning.walk(new Crossing.Tuple(List.of(count, count)));
        Manifest.FunctionCrossing crossing = Manifest.read(Documents.library(Documents.FUNCTIONS,
                into.resolve("functions")).manifest()).modules().getFirst().functions().getFirst();
        owning.walk(new Crossing.Callable(List.of(count), count, crossing, "\\Acme\\Binding",
                crossing.implement() + "#0"));

        assertThat(owning.wrong).isEmpty();
        assertThat(Owning.collectionsIn("souther.bindings.php")).contains(
                PhpBindings.Generated.class.getName() + ".files",
                Crossing.OneOf.class.getName() + ".members");
        assertThat(owning.asked).containsAll(Owning.collectionsIn("souther.bindings.php"));
    }
}
