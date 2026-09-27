//! The five behaviors the model leaves to a host (`loadProduct`, `loadCart`, `saveItem`,
//! `priceCart`, `saveOrder`), each a struct borrowing the transaction of the request it answers in
//! and implementing the trait the binding generates for the behavior.

mod load_cart;
mod load_product;
mod price_cart;
mod save_item;
mod save_order;

pub use load_cart::SqlLoadCart;
pub use load_product::SqlLoadProduct;
pub use price_cart::SqlPriceCart;
pub use save_item::SqlSaveItem;
pub use save_order::SqlSaveOrder;

use model::{Construction, Failure, HostError, Reading};

/// The value a constructor made. One the model refused is the implementation's own failure: a row
/// the model will not take is not an answer.
fn made<T>(construction: Result<Construction<T>, Failure>) -> Result<T, HostError> {
    Ok(construction?.into_result()?)
}

/// The value a decoder read, on the same terms.
fn read<T>(reading: Result<Reading<T>, Failure>) -> Result<T, HostError> {
    Ok(reading?.into_result()?)
}
