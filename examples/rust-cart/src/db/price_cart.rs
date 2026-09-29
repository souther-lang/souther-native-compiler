use model::com::example::cart::domain::{
    PriceCart, PricedCart, PricedCartOrProductNotFoundOrSaleEnded as Priced, ProductNotFound,
    SaleEnded, UserId,
};
use model::{HostError, Run};
use rusqlite::Connection;
use serde_json::json;

use super::{made, read};

/// `priceCart` over SQLite. It reads every line of the cart with its product in one query, sees
/// that each product is there and on sale, and answers the lines with their prices as a
/// `PricedCart`. A product that is gone or no longer on sale ends it with the model's own case, the
/// first such line in the order of the product ids deciding which.
///
/// Deciding that for each line stays here, in the implementation: the model has no traverse, and a
/// fold cannot call another injected behavior. The products are joined to the lines rather than
/// asked for one by one, so a cart is one query however many lines it has.
pub struct SqlPriceCart<'tx>(pub &'tx Connection);

impl PriceCart for SqlPriceCart<'_> {
    fn apply<'run>(
        &self,
        run: &mut Run<'run>,
        user_id: UserId<'run>,
    ) -> Result<Priced<'run>, HostError> {
        let mut items = self.0.prepare(
            "SELECT ci.product_id, ci.quantity, p.on_sale, p.price
             FROM cart_item ci
             JOIN cart c ON c.cart_id = ci.cart_id
             LEFT JOIN product p ON p.product_id = ci.product_id
             WHERE c.user_id = ?1
             ORDER BY ci.product_id",
        )?;

        let mut lines = Vec::new();
        for item in items.query_map([user_id.value()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<bool>>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })? {
            let (product_id, quantity, on_sale, price) = item?;
            let (Some(on_sale), Some(price)) = (on_sale, price) else {
                return Ok(Priced::ProductNotFound(made(ProductNotFound::new(run))?));
            };
            if !on_sale {
                return Ok(Priced::SaleEnded(made(SaleEnded::new(run))?));
            }
            lines
                .push(json!({ "productId": product_id, "quantity": quantity, "unitPrice": price }));
        }
        Ok(Priced::PricedCart(read(PricedCart::decode(
            run,
            &json!({ "lines": lines }).to_string(),
        ))?))
    }
}
