use model::com::example::cart::domain::{ItemAdded, PendingItem, SaveItem};
use model::{HostError, Run};
use rusqlite::Connection;
use uuid::Uuid;

use super::made;

/// `saveItem` over SQLite. It takes the cart and the item out of the `PendingItem` the model built,
/// adds the quantity to the row already there or inserts one, and answers what it wrote.
///
/// A `PendingItem` exists only where the capacity holds, so nothing here checks it again.
pub struct SqlSaveItem<'tx>(pub &'tx Connection);

impl SaveItem for SqlSaveItem<'_> {
    fn apply<'run>(
        &self,
        run: &mut Run<'run>,
        pending: PendingItem<'run>,
    ) -> Result<ItemAdded<'run>, HostError> {
        let cart_id = pending.cart().id().value();
        let item = pending.item();
        let product_id = item.productId().value();
        let quantity = item.quantity().value();

        let updated = self.0.execute(
            "UPDATE cart_item SET quantity = quantity + ?1 WHERE cart_id = ?2 AND product_id = ?3",
            (quantity, &cart_id, &product_id),
        )?;
        if updated == 0 {
            self.0.execute(
                "INSERT INTO cart_item (cart_item_id, cart_id, product_id, quantity)
                 VALUES (?1, ?2, ?3, ?4)",
                (Uuid::new_v4().to_string(), &cart_id, &product_id, quantity),
            )?;
        }

        made(ItemAdded::new(run, item.productId(), item.quantity()))
    }
}
