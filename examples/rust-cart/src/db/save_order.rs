use model::com::example::cart::domain::{Order, OrderPlaced, OrdererCase, SaveOrder};
use model::{HostError, Run};
use rusqlite::Connection;
use uuid::Uuid;

use super::made;

/// `saveOrder` over SQLite: one row for the order and one for each of its lines. The orderer is
/// laid out flat by which case it is, since an individual and a corporation fill different columns.
pub struct SqlSaveOrder<'tx>(pub &'tx Connection);

impl SaveOrder for SqlSaveOrder<'_> {
    fn apply<'run>(
        &self,
        run: &mut Run<'run>,
        order: Order<'run>,
    ) -> Result<OrderPlaced<'run>, HostError> {
        let order_id = order.id().value();
        let (kind, email, name, company_name, corporate_number) = match order.orderer().case() {
            OrdererCase::Individual(it) => (
                "individual",
                it.email().value(),
                Some(it.name()),
                None,
                None,
            ),
            OrdererCase::Corporation(it) => (
                "corporation",
                it.email().value(),
                None,
                Some(it.companyName()),
                Some(it.corporateNumber()),
            ),
        };
        let charge = order.charge();

        self.0.execute(
            "INSERT INTO orders (order_id, user_id, subtotal, discount, total, orderer_type,
                                 orderer_email, orderer_name, orderer_company_name,
                                 orderer_corporate_number)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                &order_id,
                order.userId().value(),
                charge.subtotal().value(),
                charge.discount().value(),
                charge.total().value(),
                kind,
                email,
                name,
                company_name,
                corporate_number,
            ],
        )?;

        let mut line = self.0.prepare(
            "INSERT INTO order_line (order_line_id, order_id, product_id, quantity, unit_price)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for each in order.lines() {
            line.execute((
                Uuid::new_v4().to_string(),
                &order_id,
                each.productId().value(),
                each.quantity().value(),
                each.unitPrice().value(),
            ))?;
        }

        made(OrderPlaced::new(run, order))
    }
}
