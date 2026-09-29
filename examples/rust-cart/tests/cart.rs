//! The HTTP contract every host of the cart example keeps, over the native library and SQLite: 201
//! where an item is added or an order placed, 422 for a business case the model answers, 400 for
//! an input that does not decode. The capacity is the 10000 `PendingItem` states in cart.sou. An
//! order and a quotation come back as the model writes them.
//!
//! Each test starts from a database of its own, seeded as the application seeds it.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use model::Library;
use rusqlite::Connection;
use rust_cart::App;
use serde_json::{Value, json};
use tower::ServiceExt;

const USER: &str = "11111111-1111-1111-1111-111111111111";
const ON_SALE: &str = "33333333-3333-3333-3333-333333333333";
const OFF_SALE: &str = "44444444-4444-4444-4444-444444444444";

struct Cart(Router);

struct Answer {
    status: StatusCode,
    body: Value,
}

impl Cart {
    fn new() -> Self {
        Cart::over(Connection::open_in_memory().unwrap())
    }

    fn over(connection: Connection) -> Self {
        // SAFETY: the library bin/build wrote beside the binding this test was compiled against.
        let library = unsafe { Library::load(rust_cart::library()) }.expect("run bin/build first");
        let app = App::new(library, connection).unwrap();
        Cart(rust_cart::router(app))
    }

    async fn add_item(&self, user_id: &str, product_id: &str, quantity: i64) -> Answer {
        self.post(
            "/carts/items",
            json!({ "userId": user_id, "productId": product_id, "quantity": quantity }).to_string(),
        )
        .await
    }

    async fn checkout(&self, path: &str, user_id: &str, orderer: Value) -> Answer {
        self.post(
            path,
            json!({ "userId": user_id, "orderer": orderer }).to_string(),
        )
        .await
    }

    async fn post(&self, path: &str, body: String) -> Answer {
        self.send(Request::post(path).body(Body::from(body)).unwrap())
            .await
    }

    async fn get(&self, uri: &str) -> Answer {
        self.send(Request::get(uri).body(Body::empty()).unwrap())
            .await
    }

    async fn send(&self, request: Request<Body>) -> Answer {
        let response = self.0.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        Answer { status, body }
    }
}

fn individual() -> Value {
    json!({ "type": "Individual", "email": "Taro@Example.com ", "name": "山田太郎" })
}

fn corporation() -> Value {
    json!({ "type": "Corporation", "email": "info@acme.co.jp", "companyName": "Acme株式会社",
            "corporateNumber": "1234567890123" })
}

fn with(mut object: Value, key: &str, value: Value) -> Value {
    object[key] = value;
    object
}

fn paths(answer: &Answer) -> Vec<&str> {
    answer.body["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["path"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn an_item_on_sale_is_added() {
    let cart = Cart::new();

    assert_eq!(
        cart.add_item(USER, ON_SALE, 8).await.status,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn a_quantity_over_the_capacity_is_422() {
    let answer = Cart::new().add_item(USER, ON_SALE, 10001).await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer.body, json!({ "error": "cart_full" }));
}

#[tokio::test]
async fn a_total_exactly_at_the_capacity_is_added() {
    let cart = Cart::new();

    assert_eq!(
        cart.add_item(USER, ON_SALE, 9998).await.status,
        StatusCode::CREATED
    );
    assert_eq!(
        cart.add_item(USER, ON_SALE, 2).await.status,
        StatusCode::CREATED
    );
    assert_eq!(
        cart.add_item(USER, ON_SALE, 1).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn an_item_no_longer_on_sale_is_422() {
    let answer = Cart::new().add_item(USER, OFF_SALE, 1).await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer.body, json!({ "error": "sale_ended" }));
}

#[tokio::test]
async fn a_product_nobody_sells_is_422() {
    let answer = Cart::new()
        .add_item(USER, "55555555-5555-5555-5555-555555555555", 1)
        .await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer.body, json!({ "error": "product_not_found" }));
}

#[tokio::test]
async fn an_id_that_is_not_a_uuid_is_400_with_raohs_issue() {
    let answer = Cart::new().add_item("not-a-uuid", ON_SALE, 1).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.body["issues"][0]["path"], "/userId");
    assert_eq!(answer.body["issues"][0]["code"], "invalid_format");
    assert_eq!(answer.body["issues"][0]["meta"], json!({}));
    assert!(answer.body["errors"].is_object());
}

#[tokio::test]
async fn an_id_is_a_uuid_as_this_api_writes_one() {
    // Upper case is written in lower case; a UUID in another notation is not how an id is written.
    let cart = Cart::new();

    let upper = cart.add_item(&USER.to_uppercase(), ON_SALE, 1).await;
    let braced = cart.add_item(&format!("{{{USER}}}"), ON_SALE, 1).await;
    let bare = cart.add_item(&USER.replace('-', ""), ON_SALE, 1).await;

    assert_eq!(upper.status, StatusCode::CREATED);
    assert_eq!(
        (braced.status, paths(&braced)),
        (StatusCode::BAD_REQUEST, vec!["/userId"])
    );
    assert_eq!(
        (bare.status, paths(&bare)),
        (StatusCode::BAD_REQUEST, vec!["/userId"])
    );
}

#[tokio::test]
async fn a_quantity_of_none_is_400() {
    let answer = Cart::new().add_item(USER, ON_SALE, 0).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(paths(&answer), ["/quantity"]);
}

#[tokio::test]
async fn a_body_that_is_not_json_is_400() {
    let answer = Cart::new().post("/carts/items", "{".to_owned()).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_added_item_is_listed() {
    let cart = Cart::new();
    cart.add_item(USER, ON_SALE, 3).await;

    let answer = cart.get(&format!("/carts/items?userId={USER}")).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.body,
        json!({ "total": 1, "page": 0, "size": 20,
                "items": [{ "productId": ON_SALE, "quantity": 3 }] })
    );
}

#[tokio::test]
async fn an_individual_checks_out_with_the_discount() {
    let cart = Cart::new();
    let user = "11111111-1111-1111-1111-111111111112";
    // 1200 x 8 = 9600, which is at least 5000: 10% off is 960, and the total 8640.
    cart.add_item(user, ON_SALE, 8).await;

    let answer = cart.checkout("/carts/checkout", user, individual()).await;

    assert_eq!(answer.status, StatusCode::CREATED);
    let order = answer.body;
    assert_eq!(order["userId"], user);
    assert_eq!(
        order["orderer"],
        json!({ "type": "Individual", "email": "taro@example.com", "name": "山田太郎" })
    );
    assert_eq!(
        order["charge"],
        json!({ "subtotal": 9600, "discount": 960, "total": 8640 })
    );
    assert_eq!(
        order["lines"],
        json!([{ "productId": ON_SALE, "quantity": 8, "unitPrice": 1200 }])
    );
}

#[tokio::test]
async fn a_corporation_checks_out_without_the_discount_under_5000() {
    let cart = Cart::new();
    let user = "11111111-1111-1111-1111-111111111114";
    cart.add_item(user, ON_SALE, 3).await;

    let answer = cart.checkout("/carts/checkout", user, corporation()).await;

    assert_eq!(answer.status, StatusCode::CREATED);
    assert_eq!(answer.body["orderer"], corporation());
    assert_eq!(
        answer.body["charge"],
        json!({ "subtotal": 3600, "discount": 0, "total": 3600 })
    );
}

#[tokio::test]
async fn a_corporate_number_other_than_thirteen_digits_is_400() {
    let cart = Cart::new();
    let user = "11111111-1111-1111-1111-111111111115";
    cart.add_item(user, ON_SALE, 1).await;

    let orderer = with(corporation(), "corporateNumber", json!("12345"));
    let answer = cart.checkout("/carts/checkout", user, orderer).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(paths(&answer), ["/orderer/corporateNumber"]);
}

#[tokio::test]
async fn a_name_of_nothing_but_spaces_is_400() {
    // That a name is not blank is PersonName's rule, and the model's decoder reports it.
    let orderer = with(individual(), "name", json!("   "));
    let answer = Cart::new().checkout("/carts/checkout", USER, orderer).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(paths(&answer), ["/orderer/name"]);
}

#[tokio::test]
async fn a_name_is_kept_without_the_spaces_around_it() {
    // Trimming is how the boundary writes a name, not a rule the model states, so the model is
    // handed the name without them and its bound is on what it keeps.
    let cart = Cart::new();
    let user = "11111111-1111-1111-1111-111111111119";
    cart.add_item(user, ON_SALE, 1).await;
    let longest = "名".repeat(100);

    let answer = cart
        .checkout(
            "/carts/checkout",
            user,
            with(individual(), "name", json!(format!("  {longest}  "))),
        )
        .await;

    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);
    assert_eq!(answer.body["orderer"]["name"], json!(longest));
}

#[tokio::test]
async fn a_company_name_is_kept_without_the_spaces_around_it() {
    let cart = Cart::new();
    let user = "11111111-1111-1111-1111-11111111111a";
    cart.add_item(user, ON_SALE, 1).await;

    let orderer = with(corporation(), "companyName", json!("  Acme株式会社 "));
    let answer = cart.checkout("/carts/checkout", user, orderer).await;

    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);
    assert_eq!(answer.body["orderer"]["companyName"], "Acme株式会社");
}

#[tokio::test]
async fn a_member_the_boundary_refuses_does_not_keep_the_model_from_reading_the_rest() {
    // The email is not shaped like one, which the boundary finds; the corporation has no company
    // name and no corporate number, which only the model can say.
    let orderer = json!({ "type": "Corporation", "email": "not-an-email" });

    let answer = Cart::new().checkout("/carts/checkout", USER, orderer).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    let mut found = paths(&answer);
    found.sort_unstable();
    assert_eq!(
        found,
        [
            "/orderer/companyName",
            "/orderer/corporateNumber",
            "/orderer/email"
        ],
        "{}",
        answer.body
    );
}

#[tokio::test]
async fn a_member_both_refuse_is_answered_once_by_the_boundary() {
    // A name that is no text is refused by the boundary, which trims it, and by the model, which
    // reads a PersonName. The boundary's issue says what form it was not in, and is the one kept.
    let orderer = with(individual(), "name", json!(5));

    let answer = Cart::new().checkout("/carts/checkout", USER, orderer).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(paths(&answer), ["/orderer/name"], "{}", answer.body);
}

#[tokio::test]
async fn a_refused_command_keeps_nothing_it_wrote_on_the_way() {
    // loadCart makes a new user's cart row before the capacity is decided. A command the model
    // refuses keeps nothing, and one it answers keeps what it wrote.
    let database = std::env::temp_dir().join(format!("rust-cart-{}.sqlite", uuid::Uuid::new_v4()));
    let cart = Cart::over(Connection::open(&database).unwrap());
    let carts_of = |user: &str| -> i64 {
        Connection::open(&database)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM cart WHERE user_id = ?1",
                [user],
                |row| row.get(0),
            )
            .unwrap()
    };
    let refused = "11111111-1111-1111-1111-11111111111b";
    let answered = "11111111-1111-1111-1111-11111111111c";

    let full = cart.add_item(refused, ON_SALE, 10001).await;
    let added = cart.add_item(answered, ON_SALE, 1).await;

    assert_eq!(full.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(added.status, StatusCode::CREATED);
    assert_eq!((carts_of(refused), carts_of(answered)), (0, 1));
    std::fs::remove_file(&database).unwrap();
}

#[tokio::test]
async fn an_orderer_of_no_known_type_is_400() {
    let orderer = with(individual(), "type", json!("Robot"));
    let answer = Cart::new().checkout("/carts/checkout", USER, orderer).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(paths(&answer), ["/orderer/type"]);
}

#[tokio::test]
async fn a_field_the_orderers_case_has_is_missing_is_400() {
    // Which fields an individual has is the model's to say, and its decoder says it.
    let mut orderer = individual();
    orderer.as_object_mut().unwrap().remove("name");

    let answer = Cart::new().checkout("/carts/checkout", USER, orderer).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.body["issues"][0]["path"], "/orderer/name");
    assert_eq!(answer.body["issues"][0]["code"], "missing_field");
}

#[tokio::test]
async fn every_issue_of_a_request_is_answered_at_once_whichever_step_found_it() {
    // raoh finds that the user is no UUID. The model finds that a corporation has a company name
    // and a corporate number, which raoh, reading the fields that are there, has no way to know.
    let orderer = json!({ "type": "Corporation", "email": "info@acme.co.jp" });

    let answer = Cart::new()
        .checkout("/carts/checkout", "not-a-uuid", orderer)
        .await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    let mut found = paths(&answer);
    found.sort_unstable();
    assert_eq!(
        found,
        [
            "/orderer/companyName",
            "/orderer/corporateNumber",
            "/userId"
        ],
        "{}",
        answer.body
    );
}

#[tokio::test]
async fn an_empty_cart_does_not_check_out() {
    let answer = Cart::new()
        .checkout(
            "/carts/checkout",
            "11111111-1111-1111-1111-111111111113",
            individual(),
        )
        .await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer.body, json!({ "error": "empty_cart" }));
}

#[tokio::test]
async fn a_corporation_is_quoted() {
    let cart = Cart::new();
    let user = "11111111-1111-1111-1111-111111111116";
    cart.add_item(user, ON_SALE, 8).await;

    let answer = cart.checkout("/carts/quote", user, corporation()).await;

    assert_eq!(answer.status, StatusCode::OK);
    let quote = answer.body;
    assert_eq!(quote["id"].as_str().unwrap().len(), 36);
    assert_eq!(quote["orderer"]["type"], "Corporation");
    assert_eq!(
        quote["charge"],
        json!({ "subtotal": 9600, "discount": 960, "total": 8640 })
    );
    let valid_until = quote["validUntil"].as_str().unwrap();
    assert!(
        valid_until.parse::<jiff::civil::Date>().is_ok(),
        "{valid_until}"
    );
}

#[tokio::test]
async fn an_individual_is_not_quoted() {
    let cart = Cart::new();
    let user = "11111111-1111-1111-1111-111111111117";
    cart.add_item(user, ON_SALE, 2).await;

    let answer = cart.checkout("/carts/quote", user, individual()).await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        json!({ "error": "quote_for_corporations_only" })
    );
}

#[tokio::test]
async fn an_empty_cart_is_not_quoted() {
    let answer = Cart::new()
        .checkout(
            "/carts/quote",
            "11111111-1111-1111-1111-111111111118",
            corporation(),
        )
        .await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer.body, json!({ "error": "empty_cart" }));
}

/// A product on sale that `withdrawn` adds beside the seeded one.
const SECOND: &str = "33333333-3333-3333-3333-333333333334";

/// A cart over a database of its own in which the products in the buyer's cart are changed by
/// `change` once they are in it. Beside the seeded product on sale, the cart holds `SECOND`.
async fn withdrawn(buyer: &str, change: &str) -> (Cart, std::path::PathBuf) {
    let database = std::env::temp_dir().join(format!("rust-cart-{}.sqlite", uuid::Uuid::new_v4()));
    let cart = Cart::over(Connection::open(&database).unwrap());
    Connection::open(&database)
        .unwrap()
        .execute(
            "INSERT INTO product (product_id, name, on_sale, price) VALUES (?1, 'Filter Papers', 1, 300)",
            [SECOND],
        )
        .unwrap();
    assert_eq!(
        cart.add_item(buyer, ON_SALE, 1).await.status,
        StatusCode::CREATED
    );
    assert_eq!(
        cart.add_item(buyer, SECOND, 1).await.status,
        StatusCode::CREATED
    );
    Connection::open(&database)
        .unwrap()
        .execute_batch(change)
        .unwrap();
    (cart, database)
}

#[tokio::test]
async fn a_product_no_longer_on_sale_by_checkout_is_422() {
    let buyer = "11111111-1111-1111-1111-11111111111d";
    let (cart, database) = withdrawn(
        buyer,
        &format!("UPDATE product SET on_sale = 0 WHERE product_id = '{SECOND}'"),
    )
    .await;

    let answer = cart.checkout("/carts/checkout", buyer, individual()).await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer.body, json!({ "error": "sale_ended" }));
    std::fs::remove_file(&database).unwrap();
}

#[tokio::test]
async fn a_product_gone_by_checkout_is_422() {
    let buyer = "11111111-1111-1111-1111-11111111111e";
    let (cart, database) = withdrawn(
        buyer,
        &format!("DELETE FROM product WHERE product_id = '{SECOND}'"),
    )
    .await;

    let answer = cart.checkout("/carts/checkout", buyer, individual()).await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer.body, json!({ "error": "product_not_found" }));
    std::fs::remove_file(&database).unwrap();
}

#[tokio::test]
async fn the_first_line_that_cannot_be_priced_says_why() {
    // The lines are priced in the order of their products: the first ended its sale, the second is
    // gone, and the answer is the first's.
    let buyer = "11111111-1111-1111-1111-11111111111f";
    let (cart, database) = withdrawn(
        buyer,
        &format!(
            "UPDATE product SET on_sale = 0 WHERE product_id = '{ON_SALE}';
             DELETE FROM product WHERE product_id = '{SECOND}';"
        ),
    )
    .await;

    let answer = cart.checkout("/carts/quote", buyer, corporation()).await;

    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(answer.body, json!({ "error": "sale_ended" }));
    std::fs::remove_file(&database).unwrap();
}
