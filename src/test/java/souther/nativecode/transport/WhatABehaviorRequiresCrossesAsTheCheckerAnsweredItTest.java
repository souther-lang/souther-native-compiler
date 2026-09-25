package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What constructing a behavior requires injected crosses as the checker answered it, beside the
 * definition, and is not worked out again from what the definition calls: a body requires what it
 * depends on, in the order it names them, and a composition what its stages require.
 */
class WhatABehaviorRequiresCrossesAsTheCheckerAnsweredItTest {

    private static final String SHOP = """
            module shop exposing ( quote, total, priced : Int )

            behavior priceOf : (sku: String) -> Int

            behavior discountFor : (sku: String) -> Int

            behavior quote : (sku: String, count: Int) -> Int
                depends on discountFor, priceOf
            let quote (sku, count, discountFor, priceOf) = priceOf(sku) * count - discountFor(sku)

            behavior total : (sku: String) -> Int
                depends on quote
            let total (sku, quote) = quote(sku, 2)

            behavior doubled : (n: Int) -> Int
            let doubled (n) = n * 2

            behavior priced = priceOf >-> doubled
            """;

    private static String written() {
        return ProgramWriter.written(CheckedProgram.of(List.of(SHOP)));
    }

    @Test
    void aBodyRequiresWhatItDependsOnInTheOrderItNamesThem() {
        assertThat(written()).contains("\"declared\":\"shop.quote\",\"parameters\":[\"sku\",\"count\"],"
                + "\"publication\":\"published\",\"requirements\":["
                + "{\"module\":\"shop\",\"name\":\"discountFor\"},"
                + "{\"module\":\"shop\",\"name\":\"priceOf\"}]");
    }

    /** One it depends on that is constructed in turn is what it requires, and not what that requires. */
    @Test
    void aBodyRequiresABehaviorConstructedInTurnAndNotWhatThatRequires() {
        assertThat(written()).contains("\"declared\":\"shop.total\",\"parameters\":[\"sku\"],"
                + "\"publication\":\"published\","
                + "\"requirements\":[{\"module\":\"shop\",\"name\":\"quote\"}]");
    }

    /** A composition calls nothing, and requires what its stages require. */
    @Test
    void aCompositionRequiresWhatItsStagesRequire() {
        assertThat(written()).contains("\"declared\":\"shop.priced\",\"publication\":\"published\","
                + "\"requirements\":[{\"module\":\"shop\",\"name\":\"priceOf\"}]");
        assertThat(written()).contains("\"declared\":\"shop.doubled\",\"parameters\":[\"n\"],"
                + "\"publication\":\"kept\",\"requirements\":[]");
    }
}
