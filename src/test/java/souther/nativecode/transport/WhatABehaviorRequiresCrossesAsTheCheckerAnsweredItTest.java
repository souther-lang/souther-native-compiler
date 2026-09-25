package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.meta.ModulePath;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What constructing a behavior requires injected crosses as the checker answered it, on the
 * behavior's target wherever it is reached, and is not worked out again from what a definition
 * calls: a body requires what it depends on, in the order it names them, a composition what its
 * stages require, a behavior a host implements nothing, and one another build implements what that
 * build published.
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

    /** The target of {@code name} in {@code module}, from its name to the end of its object. */
    private static String target(String written, String module, String name) {
        int at = written.indexOf("{\"module\":\"" + module + "\",\"name\":\"" + name + "\",\"is\":");
        assertThat(at).as("a target for %s.%s", module, name).isNotNegative();
        return written.substring(at, written.indexOf("\"requirements\":", at));
    }

    private static String requirementsOf(String written, String module, String name) {
        int from = written.indexOf("\"requirements\":", written.indexOf(
                "{\"module\":\"" + module + "\",\"name\":\"" + name + "\",\"is\":"));
        return written.substring(from, written.indexOf("]", from) + 1);
    }

    @Test
    void aBodyRequiresWhatItDependsOnInTheOrderItNamesThem() {
        assertThat(requirementsOf(written(), "shop", "quote")).isEqualTo("\"requirements\":["
                + "{\"module\":\"shop\",\"name\":\"discountFor\"},"
                + "{\"module\":\"shop\",\"name\":\"priceOf\"}]");
    }

    /** One it depends on that is constructed in turn is what it requires, and not what that requires. */
    @Test
    void aBodyRequiresABehaviorConstructedInTurnAndNotWhatThatRequires() {
        assertThat(requirementsOf(written(), "shop", "total"))
                .isEqualTo("\"requirements\":[{\"module\":\"shop\",\"name\":\"quote\"}]");
    }

    /** A composition calls nothing, and requires what its stages require. */
    @Test
    void aCompositionRequiresWhatItsStagesRequire() {
        String written = written();
        assertThat(requirementsOf(written, "shop", "priced"))
                .isEqualTo("\"requirements\":[{\"module\":\"shop\",\"name\":\"priceOf\"}]");
        assertThat(requirementsOf(written, "shop", "doubled")).isEqualTo("\"requirements\":[]");
    }

    /**
     * A behavior a host implements requires nothing to construct, since Souther does not construct
     * one, and is told apart from one that requires nothing by how it answers.
     */
    @Test
    void aBehaviorAHostImplementsRequiresNothing() {
        String written = written();
        assertThat(target(written, "shop", "priceOf")).contains("\"is\":\"injected\"");
        assertThat(requirementsOf(written, "shop", "priceOf")).isEqualTo("\"requirements\":[]");
    }

    /** Written once, on the target, and not a second time beside the definition. */
    @Test
    void aDefinitionDoesNotSayItASecondTime() {
        assertThat(written()).contains("\"declared\":\"shop.quote\",\"parameters\":[\"sku\",\"count\"],"
                + "\"publication\":\"published\",\"body\":");
    }

    private static final String PORT = """
            module lib.port exposing ( lookUp, looked )

            behavior lookUp : (a: Int) -> Int

            behavior looked : (a: Int) -> Int
                depends on lookUp
            let looked (a, lookUp) = lookUp(a)
            """;

    private static final String PIPED = """
            module app.piped exposing ( piped : Int )

            import lib.port ( looked )

            behavior doubled : (a: Int) -> Int
            let doubled (a) = a * 2

            behavior piped = looked >-> doubled
            """;

    /**
     * A behavior another build implements requires what that build published, so a composition
     * here hands the stage what it requires whichever build implements it.
     */
    @Test
    void aBehaviorAnotherBuildImplementsRequiresWhatThatBuildPublished() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(PIPED),
                ModulePath.of(Compiler.compile(PORT))));

        assertThat(target(written, "lib.port", "looked")).contains("\"is\":\"elsewhere\"");
        assertThat(requirementsOf(written, "lib.port", "looked"))
                .isEqualTo("\"requirements\":[{\"module\":\"lib.port\",\"name\":\"lookUp\"}]");
        assertThat(requirementsOf(written, "app.piped", "piped"))
                .isEqualTo("\"requirements\":[{\"module\":\"lib.port\",\"name\":\"lookUp\"}]");
    }
}
