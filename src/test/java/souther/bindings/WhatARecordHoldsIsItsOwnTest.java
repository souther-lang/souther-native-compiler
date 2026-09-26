package souther.bindings;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every record the shared package offers owns the collections it holds, so what its constructor
 * held it to holds for as long as it is held, however it was made: a manifest's parts and a
 * shape alike. Asked of a real manifest and of every shape worked out of it, and required of every
 * collection of every record the package declares, so a record added later is asked of too.
 */
class WhatARecordHoldsIsItsOwnTest {

    /** A module with something in every collection a record of a manifest or a shape holds. */
    private static final String EVERYTHING = """
            module shop exposing ( Money, Item, Cart, Free, Paid, Settled, Found, Missing,
                                   find, settle, owing, stillOwing : Int, quote, boxed )

            data Money = Int

            data Item = { n: Int }
            data Cart = { items: List<Item> }

            data Free
            data Paid = { amount: Money }
            data Settled = Free | Paid

            data Found = { id: Int }
            data Missing

            behavior find : (id: Int) -> Found | Missing
            let find (id) = if id > 0 then Found { id = id } else Missing

            behavior settle : (paid: Int) -> Settled
            let settle (paid) = if paid > 0 then Paid { amount = Money(paid) } else Free

            behavior owing : (settled: Settled) -> Int
            let owing (settled) = match settled with
                | Free -> 0
                | Paid as p -> p.amount.value

            behavior stillOwing = settle >-> owing

            behavior priceOf : (n: Int) -> Int

            behavior quote : (n: Int) -> Int
                depends on priceOf
            let quote (n, priceOf) = priceOf(n) * 2

            let boxed = Item { n = 42 }
            """;

    @Test
    void everyCollectionARecordHoldsIsItsOwn(@TempDir Path into) throws Exception {
        Manifest manifest = Manifest.read(NativeCompiler.library(
                CheckedProgram.of(List.of(EVERYTHING)), into).manifest());
        Owning owning = new Owning();
        owning.walk(manifest.runtime());
        owning.walk(manifest.modules());
        owning.walk(shapesOf(manifest));

        assertThat(owning.wrong).isEmpty();
        assertThat(Owning.collectionsIn("souther.bindings")).contains(
                Manifest.Function.class.getName() + ".takes",
                CrossingShape.Told.class.getName() + ".cases");
        assertThat(owning.asked).containsAll(Owning.collectionsIn("souther.bindings"));
    }

    /** The manifest's own collections are its own too, and cannot be changed through. */
    @Test
    void aManifestAnswersNothingItCanBeChangedThrough(@TempDir Path into) throws Exception {
        Manifest manifest = Manifest.read(NativeCompiler.library(
                CheckedProgram.of(List.of(EVERYTHING)), into).manifest());

        assertThat(List.of(manifest.runtime(), manifest.modules(), manifest.statuses().keySet(),
                manifest.outcomes().keySet())).allSatisfy(held -> assertThat(held).isNotEmpty());
        org.assertj.core.api.Assertions.assertThatThrownBy(() -> manifest.modules().clear())
                .isInstanceOf(UnsupportedOperationException.class);
        org.assertj.core.api.Assertions.assertThatThrownBy(() -> manifest.statuses().clear())
                .isInstanceOf(UnsupportedOperationException.class);
    }

    /** Every shape a type in the manifest crosses in, both ways, and each behavior's answer. */
    private static List<CrossingShape> shapesOf(Manifest manifest) {
        List<CrossingShape> shapes = new ArrayList<>();
        for (Manifest.Module module : manifest.modules()) {
            List<Manifest.Type> types = new ArrayList<>();
            module.behaviors().forEach(it -> types.addAll(it.parameters().types()));
            module.values().forEach(it -> types.add(it.type()));
            for (Manifest.Declaration declaration : module.declarations()) {
                switch (declaration) {
                    case Manifest.Declaration.Product it ->
                            it.fields().forEach(field -> types.add(field.type()));
                    case Manifest.Declaration.Newtype it -> types.add(it.field().type());
                    default -> {
                    }
                }
            }
            for (Manifest.Type type : types) {
                shapes.add(CrossingShape.given(module, type));
                shapes.add(CrossingShape.received(module, type));
            }
            module.behaviors().forEach(it -> shapes.add(CrossingShape.received(module,
                    it.answers())));
        }
        shapes.removeIf(java.util.Objects::isNull);
        return shapes;
    }
}
