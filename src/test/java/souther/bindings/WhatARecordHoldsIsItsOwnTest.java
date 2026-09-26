package souther.bindings;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Documents;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every record the shared package offers owns the collections it holds, so what its constructor
 * held it to holds for as long as it is held, however it was made: a manifest's parts, its shapes
 * and what it says of why nothing reaches a value alike. Asked of real manifests — one of a
 * program, and one of the document of function values no program writes yet — and required of
 * every collection of every record the package declares, so a record added later is asked of too.
 */
class WhatARecordHoldsIsItsOwnTest {

    /** A module with something in every collection a record of a manifest or a shape holds. */
    private static final String EVERYTHING = """
            module shop exposing ( Money, Item, Cart, Free, Paid, Settled, Found, Missing,
                                   find, settle, owing, stillOwing : Int, quote, boxed, pair, Partial )

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

            let pair: (Int, Bool) = (3, true)

            data Partial = { count: Int, amount: Decimal }
            """;

    @Test
    void everyCollectionARecordHoldsIsItsOwn(@TempDir Path into) throws Exception {
        Manifest manifest = Manifest.read(NativeCompiler.library(
                Checked.of(List.of(EVERYTHING)), into).manifest());
        Manifest functions = Manifest.read(Documents.library(Documents.FUNCTIONS,
                into.resolve("functions")).manifest());
        Owning owning = new Owning();
        owning.walk(manifest.runtime());
        owning.walk(manifest.modules());
        owning.walk(functions.modules());

        assertThat(owning.wrong).isEmpty();
        assertThat(Owning.collectionsIn("souther.bindings")).contains(
                Manifest.Function.class.getName() + ".takes",
                Manifest.Signature.class.getName() + ".takes",
                Manifest.Refusal.class.getName() + ".path");
        assertThat(owning.asked).containsAll(Owning.collectionsIn("souther.bindings"));
    }

    /** The manifest's own collections are its own too, and cannot be changed through. */
    @Test
    void aManifestAnswersNothingItCanBeChangedThrough(@TempDir Path into) throws Exception {
        Manifest manifest = Manifest.read(NativeCompiler.library(
                Checked.of(List.of(EVERYTHING)), into).manifest());

        assertThat(List.of(manifest.runtime(), manifest.modules(), manifest.statuses().keySet(),
                manifest.outcomes().keySet())).allSatisfy(held -> assertThat(held).isNotEmpty());
        org.assertj.core.api.Assertions.assertThatThrownBy(() -> manifest.modules().clear())
                .isInstanceOf(UnsupportedOperationException.class);
        org.assertj.core.api.Assertions.assertThatThrownBy(() -> manifest.statuses().clear())
                .isInstanceOf(UnsupportedOperationException.class);
    }
}
