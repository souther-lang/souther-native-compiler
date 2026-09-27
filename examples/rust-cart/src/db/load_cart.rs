use model::com::example::cart::domain::{Cart, LoadCart, UserId};
use model::{HostError, Run};
use rusqlite::Connection;
use serde_json::json;
use uuid::Uuid;

use super::read;

/// `loadCart` over SQLite. It makes sure the user has a cart row, then reads the cart with the
/// total quantity of what is in it in one aggregate query: the items themselves are not loaded, and
/// the total is what the capacity rule needs.
///
/// Making the row is a write before the command has been decided: a `Cart` has an id, and the row
/// is where a new user's cart gets one. A command the model refuses after it (`CartFull`) keeps
/// nothing, since its route answers [`Outcome::Rollback`](crate::Outcome::Rollback).
pub struct SqlLoadCart<'tx>(pub &'tx Connection);

impl LoadCart for SqlLoadCart<'_> {
    fn apply<'run>(
        &self,
        run: &mut Run<'run>,
        user_id: UserId<'run>,
    ) -> Result<Cart<'run>, HostError> {
        let user_id = user_id.value();
        self.0.execute(
            "INSERT OR IGNORE INTO cart (cart_id, user_id) VALUES (?1, ?2)",
            (Uuid::new_v4().to_string(), &user_id),
        )?;
        let (cart_id, total) = self.0.query_row(
            "SELECT c.cart_id, COALESCE(SUM(ci.quantity), 0)
             FROM cart c
             LEFT JOIN cart_item ci ON ci.cart_id = c.cart_id
             WHERE c.user_id = ?1
             GROUP BY c.cart_id",
            [&user_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )?;
        read(Cart::decode(
            run,
            &json!({ "id": cart_id, "currentQuantity": total }).to_string(),
        ))
    }
}
