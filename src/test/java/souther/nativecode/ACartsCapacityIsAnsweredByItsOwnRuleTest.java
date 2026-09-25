package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * {@code addItemToCart} from the cart model #58 ports, as it is written there, and the capacity it
 * is written against.
 *
 * <p>The capacity limit is a clause of {@code PendingItem}, and {@code CartFull} is what the
 * behavior answers where it does not hold. The behavior compiles here as the model writes it. The
 * model's own rows state identifiers such as {@code UserId("u-1")}, which a row cannot state here
 * until #61; so the limit is put to rows through a behavior over numbers that attempts the same
 * {@code PendingItem}, with a cart at the limit and one over it. The one over it is answered by the
 * arm naming the clause, and not by the run ending.
 */
class ACartsCapacityIsAnsweredByItsOwnRuleTest {

    /** The model's types, which both modules below declare. */
    private static final String TYPES = """
            data Money = Int
            data Quantity = Int
                invariant value > 0
            data TotalQuantity = Int
                invariant value >= 0
            data CartId = String
                invariant String.length(value) > 0
            data ProductId = String
                invariant String.length(value) > 0
            data UserId = String
                invariant String.length(value) > 0

            data Product = { id: ProductId, onSale: Bool, price: Money }
            data CartItem = { productId: ProductId, quantity: Quantity }
            data Cart = { id: CartId, currentQuantity: TotalQuantity }

            data PendingItem = { cart: Cart, item: CartItem }
                invariant withinCapacity = cart.currentQuantity.value + item.quantity.value <= 10000
            data ItemAdded = { productId: ProductId, quantity: Quantity }

            data ProductNotFound
            data SaleEnded
            data CartFull
            """;

    private static final String CART = "module com.example.cart.domain\n\n" + TYPES + """

            behavior loadProduct : (productId: ProductId) -> Product | ProductNotFound
                constructs Product
            behavior loadCart : (userId: UserId) -> Cart
                constructs Cart
            behavior saveItem : (pending: PendingItem) -> ItemAdded
                constructs ItemAdded

            behavior addItemToCart : (userId: UserId, productId: ProductId, quantity: Quantity) -> ItemAdded | ProductNotFound | SaleEnded | CartFull
                depends on loadProduct, loadCart, saveItem
            let addItemToCart (userId, productId, quantity, loadProduct, loadCart, saveItem) =
                match loadProduct(productId) with
                    | ProductNotFound -> ProductNotFound
                    | Product { id, onSale, price } -> {
                        guard onSale else SaleEnded
                        let c = loadCart(userId)
                        guard PendingItem { cart = c, item = CartItem { productId = productId, quantity = quantity } } as pending
                            else | withinCapacity -> CartFull
                        saveItem(pending)
                    }
            """;

    /**
     * The same {@code PendingItem}, attempted from numbers a row can state: what a cart holds and
     * what is added to it.
     */
    private static final String FITTING = "module com.example.cart.fitting\n\n" + TYPES + """

            behavior fits : (current: Int, quantity: Int) -> Bool
            let fits (current, quantity) = {
                let c = Cart { id = CartId("c"), currentQuantity = TotalQuantity(current) }
                guard PendingItem { cart = c, item = CartItem { productId = ProductId("p"), quantity = Quantity(quantity) } } as pending
                    else | withinCapacity -> false
                pending.item.quantity.value == quantity
            }

            example fits
                | "an empty cart" : (0, 2) -> true
                | "the total exactly at the limit" : (9998, 2) -> true
                | "the total over the limit" : (9999, 2) -> false
            """;

    @Test
    void addingAnItemCompilesAsTheModelWritesIt() throws Exception {
        assertThat(NativeCompiler.compile(CheckedProgram.of(List.of(CART)))).isNotEmpty();
    }

    @Test
    void aCartAtTheLimitTakesTheItemAndOneOverItIsFull() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(FITTING);
    }

    /** The capacity is decided by the type's clause, which is the one place it is written. */
    @Test
    void theCapacityIsAskedOfPendingItemAndWrittenNowhereElse() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(CART)));

        assertThat(written)
                .contains("{\"core\":\"attempt\",\"declared\":\"com.example.cart.domain.PendingItem\"")
                .contains("\"departures\":[{\"clause\":\"withinCapacity\"");
    }
}
