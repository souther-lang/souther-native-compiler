//! The HTTP boundary. A body is read into the arguments of a behavior, the behavior is called, and
//! a `match` over what it answered picks the response. What an order or a quotation is written as
//! is the model's own encoding of it.
//!
//! The answer of a behavior is an enum of exactly the cases the model says it can answer, so each
//! `match` here is checked for the cases it leaves out when it is compiled. A case added to the
//! model stops this compiling until a route says what it is answered with.

pub mod request;
pub mod response;

use std::collections::HashMap;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::Response;
use axum::routing::post;
use model::Construction;
use model::com::example::cart::domain::{
    CartFullOrItemAddedOrProductNotFoundOrSaleEnded as Added,
    EmptyCartOrOrderPlacedOrProductNotFoundOrSaleEnded as Placed,
    EmptyCartOrProductNotFoundOrQuotationOrSaleEnded as Quoted, OrderId, OrdererCase, QuoteId,
};
use rusqlite::Connection;
use serde_json::json;
use uuid::Uuid;

use crate::App;

/// The routes of the cart over `app`.
pub fn router(app: App) -> Router {
    Router::new()
        .route("/carts/items", post(add_item).get(list_items))
        .route("/carts/checkout", post(checkout))
        .route("/carts/quote", post(quote))
        .with_state(app)
}

/// `POST /carts/items`
async fn add_item(State(app): State<App>, body: String) -> Response {
    app.handle(move |behaviors, run, _| {
        let (user_id, product_id, quantity) = match request::add_item(run, &body)? {
            Ok(arguments) => arguments,
            Err(issues) => return Ok(response::bad_request(&issues)),
        };
        Ok(
            match behaviors
                .add_item_to_cart
                .call(run, user_id, product_id, quantity)?
            {
                Added::ItemAdded(_) => response::created(None),
                Added::ProductNotFound(_) => response::unprocessable("product_not_found"),
                Added::SaleEnded(_) => response::unprocessable("sale_ended"),
                Added::CartFull(_) => response::unprocessable("cart_full"),
            },
        )
    })
    .await
}

/// `POST /carts/checkout`
async fn checkout(State(app): State<App>, body: String) -> Response {
    app.handle(move |behaviors, run, _| {
        let (user_id, orderer) = match request::checkout(run, &body)? {
            Ok(arguments) => arguments,
            Err(issues) => return Ok(response::bad_request(&issues)),
        };
        let Construction::Value(order_id) = OrderId::new(run, &Uuid::new_v4().to_string())? else {
            unreachable!("an OrderId is any text but the empty one, and a UUID's is not empty")
        };
        Ok(
            match behaviors
                .place_order
                .call(run, order_id, user_id, orderer)?
            {
                Placed::OrderPlaced(placed) => response::created(Some(placed.order().encode())),
                Placed::EmptyCart(_) => response::unprocessable("empty_cart"),
                Placed::SaleEnded(_) => response::unprocessable("sale_ended"),
                Placed::ProductNotFound(_) => response::unprocessable("product_not_found"),
            },
        )
    })
    .await
}

/// `POST /carts/quote`, for a corporation only.
async fn quote(State(app): State<App>, body: String) -> Response {
    app.handle(move |behaviors, run, _| {
        let (user_id, orderer) = match request::checkout(run, &body)? {
            Ok(arguments) => arguments,
            Err(issues) => return Ok(response::bad_request(&issues)),
        };
        // issueQuote takes a Corporation, so the orderer is narrowed here, over both of its cases.
        let corporation = match orderer.case() {
            OrdererCase::Corporation(corporation) => corporation,
            OrdererCase::Individual(_) => {
                return Ok(response::unprocessable("quote_for_corporations_only"));
            }
        };
        let Construction::Value(quote_id) = QuoteId::new(run, &Uuid::new_v4().to_string())? else {
            unreachable!("a QuoteId is any text but the empty one, and a UUID's is not empty")
        };
        let valid_until = jiff::Zoned::now().date() + jiff::Span::new().days(30);
        Ok(
            match behaviors.issue_quote.call(
                run,
                quote_id,
                user_id,
                corporation,
                &valid_until.to_string(),
            )? {
                Quoted::Quotation(quotation) => response::ok(quotation.encode()),
                Quoted::EmptyCart(_) => response::unprocessable("empty_cart"),
                Quoted::SaleEnded(_) => response::unprocessable("sale_ended"),
                Quoted::ProductNotFound(_) => response::unprocessable("product_not_found"),
            },
        )
    })
    .await
}

/// `GET /carts/items?userId=…&page=…&size=…`
///
/// A listing for a screen, which the model has no behavior for and needs none: the rows are read
/// and written out as they are. Only the user is the model's, checked as every other input is.
async fn list_items(
    State(app): State<App>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    app.handle(move |_, run, database| {
        let user_id = match request::user_id(run, query.get("userId").map(String::as_str))? {
            Ok(user_id) => user_id.value(),
            Err(issues) => return Ok(response::bad_request(&issues)),
        };
        let number = |name: &str, least: i64, otherwise: i64| {
            query
                .get(name)
                .and_then(|it| it.parse().ok())
                .unwrap_or(otherwise)
                .max(least)
        };
        let (page, size) = (number("page", 0, 0), number("size", 1, 20));
        Ok(match listing(database, &user_id, page, size) {
            Ok(listing) => response::ok(listing.to_string()),
            Err(error) => {
                eprintln!("the listing could not be read: {error}");
                response::internal()
            }
        })
    })
    .await
}

fn listing(
    database: &Connection,
    user_id: &str,
    page: i64,
    size: i64,
) -> rusqlite::Result<serde_json::Value> {
    let total: i64 = database.query_row(
        "SELECT COUNT(*) FROM cart_item ci JOIN cart c ON c.cart_id = ci.cart_id WHERE c.user_id = ?1",
        [user_id],
        |row| row.get(0),
    )?;
    let mut select = database.prepare(
        "SELECT ci.product_id, ci.quantity
         FROM cart_item ci JOIN cart c ON c.cart_id = ci.cart_id
         WHERE c.user_id = ?1
         ORDER BY ci.product_id
         LIMIT ?2 OFFSET ?3",
    )?;
    let items = select
        .query_map((user_id, size, page.saturating_mul(size)), |row| {
            Ok(json!({ "productId": row.get::<_, String>(0)?, "quantity": row.get::<_, i64>(1)? }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "total": total, "page": page, "size": size, "items": items }))
}
