use model::com::example::cart::domain::{
    LoadProduct, Product, ProductId, ProductNotFound, ProductOrProductNotFound,
};
use model::{HostError, Run};
use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

use super::{made, read};

/// `loadProduct` over SQLite. The row is handed to `Product`'s decoder under the type's field
/// names, and the model checks `ProductId`'s invariant again: this is where the database meets the
/// model. No row is the model's own `ProductNotFound`.
pub struct SqlLoadProduct<'tx>(pub &'tx Connection);

impl LoadProduct for SqlLoadProduct<'_> {
    fn apply<'run>(
        &self,
        run: &mut Run<'run>,
        product_id: ProductId<'run>,
    ) -> Result<ProductOrProductNotFound<'run>, HostError> {
        let row = self
            .0
            .query_row(
                "SELECT product_id, on_sale, price FROM product WHERE product_id = ?1",
                [product_id.value()],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "onSale": row.get::<_, bool>(1)?,
                        "price": row.get::<_, i64>(2)?,
                    }))
                },
            )
            .optional()?;
        Ok(match row {
            Some(row) => {
                ProductOrProductNotFound::Product(read(Product::decode(run, &row.to_string()))?)
            }
            None => ProductOrProductNotFound::ProductNotFound(made(ProductNotFound::new(run))?),
        })
    }
}
