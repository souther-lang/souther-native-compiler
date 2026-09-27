use model::com::example::cart::domain::{
    PriceCart, PricedCart, PricedCartOrProductNotFoundOrSaleEnded as Priced, ProductNotFound,
    SaleEnded, UserId,
};
use model::{HostError, Run};
use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

use super::{made, read};

/// `priceCart` over SQLite. It reads every line of the cart, looks each product up to see that it
/// is there and on sale, and answers the lines with their prices as a `PricedCart`. A product that
/// is gone or no longer on sale ends it with the model's own case.
///
/// The loop that asks for each line's product stays here, in the implementation: the model has no
/// traverse, and a fold cannot call another injected behavior.
pub struct SqlPriceCart<'tx>(pub &'tx Connection);

impl PriceCart for SqlPriceCart<'_> {
    fn apply<'run>(
        &self,
        run: &mut Run<'run>,
        user_id: UserId<'run>,
    ) -> Result<Priced<'run>, HostError> {
        let mut items = self.0.prepare(
            "SELECT ci.product_id, ci.quantity
             FROM cart_item ci
             JOIN cart c ON c.cart_id = ci.cart_id
             WHERE c.user_id = ?1
             ORDER BY ci.product_id",
        )?;
        let mut product = self
            .0
            .prepare("SELECT on_sale, price FROM product WHERE product_id = ?1")?;

        let mut lines = Vec::new();
        for item in items.query_map([user_id.value()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })? {
            let (product_id, quantity) = item?;
            let found = product
                .query_row([&product_id], |row| {
                    Ok((row.get::<_, bool>(0)?, row.get::<_, i64>(1)?))
                })
                .optional()?;
            let Some((on_sale, price)) = found else {
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
