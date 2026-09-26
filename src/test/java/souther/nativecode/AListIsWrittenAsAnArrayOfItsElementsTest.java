package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A list in the external form is an array of its elements, each written and read as it would be
 * anywhere else, and a mistake inside one is answered at the element's own place.
 *
 * <p>The place is the element's index below the list's, as a JSON Pointer writes it, so a host
 * told {@code /lines/2/quantity} knows which line of which order it was. Every element is read
 * whatever the one before it was, as every field of an object is.
 */
class AListIsWrittenAsAnArrayOfItsElementsTest {

    private static final String WIRE = """
            module ordering exposing ( OrderLine, Order, Grid )

            data OrderLine = { sku: String, quantity: Int }
                invariant positive = quantity > 0

            data Order = { number: Int, lines: List<OrderLine> }

            data Grid = { rows: List<List<Int>> }
            """;

    @Test
    void aListIsReadAndWrittenAsAnArray() throws Exception {
        String harness = new Decoding()
                .type("ordering", "Order").type("ordering", "Grid")
                .row("order", "Order",
                        "{\"number\":1,\"lines\":[{\"sku\":\"a\",\"quantity\":2},{\"sku\":\"b\",\"quantity\":1}]}")
                .row("order empty", "Order", "{\"number\":1,\"lines\":[]}")
                .row("order bad line", "Order",
                        "{\"number\":1,\"lines\":[{\"sku\":\"a\",\"quantity\":1},{\"sku\":\"b\",\"quantity\":1},"
                                + "{\"sku\":\"c\",\"quantity\":\"x\"}]}")
                .row("order every bad line", "Order",
                        "{\"number\":1,\"lines\":[{\"quantity\":1},{\"sku\":\"b\",\"quantity\":0},3]}")
                .row("order lines object", "Order", "{\"number\":1,\"lines\":{}}")
                .row("order no lines", "Order", "{\"number\":1}")
                .row("grid", "Grid", "{\"rows\":[[1,2],[],[3]]}")
                .row("grid bad cell", "Grid", "{\"rows\":[[1,2],[3,true]]}")
                .harness();

        assertThat(AValueIsReadFromTheFormItIsWrittenInTest.run(
                Checked.of(List.of(WIRE)), harness)).isEqualTo("""
                order: value {"number":1,"lines":[{"sku":"a","quantity":2},{"sku":"b","quantity":1}]}
                order empty: value {"number":1,"lines":[]}
                order bad line: issues [@/lines/2/quantity type_mismatch actual=string expected=Int]
                order every bad line: issues [@/lines/0/sku missing_field actual=nothing expected=a field] [@/lines/1 invariant_violation module=ordering type=OrderLine clause=positive] [@/lines/2 type_mismatch actual=number expected=an object]
                order lines object: issues [@/lines type_mismatch actual=object expected=an array]
                order no lines: issues [@/lines missing_field actual=nothing expected=a field]
                grid: value {"rows":[[1,2],[],[3]]}
                grid bad cell: issues [@/rows/1/1 type_mismatch actual=boolean expected=Int]
                """);
    }
}
