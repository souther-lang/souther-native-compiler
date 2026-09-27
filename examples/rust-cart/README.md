# rust-cart

The cart example every host binding of a Souther library is shown with, written as a Rust
application. The domain is `examples/cart-model`, the model every host's cart runs, compiled by this
repository into a shared library and its Rust binding. What is written in Rust is the boundaries
around it: HTTP served by [axum](https://github.com/tokio-rs/axum), JSON decoded with
[raoh](https://github.com/kawasima/raoh-rust) into the model's values, and the behaviors the model
asks a host for implemented over SQLite with [rusqlite](https://github.com/rusqlite/rusqlite).

## What is in it

The rules of the cart are in `examples/cart-model/cart.sou` and nowhere else: the capacity of 10000, which a
`PendingItem` holds or is not built; the 10% discount at 5000 and above; that an empty cart is not
ordered and a quotation is for a corporation. Its `example` rows state what each behavior answers,
and they run when it is built, so a rule that stopped holding stops the build before Cargo is
reached.

The Rust is three modules. `src/http` reads a request into a behavior's arguments, calls the
behavior, and picks the response with a `match` over what it answered:

```rust
let (user_id, orderer) = match request::checkout(run, &body)? {
    Ok(arguments) => arguments,
    Err(issues) => return Ok(response::bad_request(&issues)),
};
// ...
match behaviors.place_order.call(run, order_id, user_id, orderer)? {
    Placed::OrderPlaced(placed) => response::created(Some(placed.order().encode())),
    Placed::EmptyCart(_) => response::unprocessable("empty_cart"),
    Placed::SaleEnded(_) => response::unprocessable("sale_ended"),
    Placed::ProductNotFound(_) => response::unprocessable("product_not_found"),
}
```

`src/db` holds the five behaviors the model leaves to a host (`loadProduct`, `loadCart`, `saveItem`,
`priceCart`, `saveOrder`), each a struct implementing the trait the binding generates for it.
`src/app.rs` binds the three composed behaviors (`addItemToCart`, `placeOrder`, `issueQuote`) to
those for each request, and runs the request.

There is no entity, no DTO, no repository and no view model. The types a request is read into are
the binding's, and what an order or a quotation is written back as is the model's own encoding of
it, `encode()`, so there is no second description of an order to keep in step with the first.

## What Rust has already, and what the model adds

Rust already says much of what a model says. A newtype with a private field and a checked
constructor keeps an invalid `Quantity` from existing, an enum with an exhaustive `match` lists the
cases an answer can be, and raoh reads a request into such types and reports every issue it finds
with its JSON Pointer. None of that is what this example is for.

What a type states is written once. `PendingItem`'s rule relates two fields of two types:

```
data PendingItem = { cart: Cart, item: CartItem }
    invariant withinCapacity = cart.currentQuantity.value + item.quantity.value <= 10000
```

and `addItemToCart` turns that rule into a business answer where it builds one:

```
guard PendingItem { cart = c, item = CartItem { productId = productId, quantity = quantity } } as pending
    else | withinCapacity -> CartFull
```

In Rust by hand, that is a constructor answering an error of its own, a decoder that calls the
constructor and reports its error as an issue at the right path, and a `match` in the service
mapping that error to `CartFull`: three places that have to agree on one rule, and a test for each.
Here the binding's `PendingItem::new` refuses the value with an `invariant_violation`, its `decode`
reports the same at the value's path, and the behavior answers `CartFull`, all from the two lines
above.

The rules are checked without a database. `fake loadCart` and the other fakes in `cart.sou` stand
for the injected behaviors, and the `example` rows run `addItemToCart`, `placeOrder` and
`issueQuote` over them when `bin/build` compiles the model. Nothing here mocks a trait.

The model is the same on every host. `examples/php-cart` and this one build one file,
`examples/cart-model/cart.sou`, and an order is written in the one encoding the model gives it, so
a client does not know which host answered. What a field of a request may hold is in that file too:
a corporate number of thirteen digits is `CorporateNumber`'s rule, and neither host says it again.

What another host checks when it runs, rustc checks here. A value of the model lives in an arena
that belongs to the run it was made in, and the run ends with the request. PHP throws `Expired` when
such a value is used after its run; Rust does not compile it. `tests/ui` holds each mistake beside
what rustc says about it:

| The mistake | What rustc says |
|---|---|
| a value kept after its request, as a cache would keep one | E0521, borrowed data escapes outside of closure |
| a value handed to another thread | E0277, cannot be sent between threads safely |
| a run made `async`, to wait on something inside it | lifetime may not live long enough |
| a transaction committed while an implementation reading through it can still be called | E0505, cannot move out of `transaction` because it is borrowed |
| a behavior bound without one of the behaviors it depends on | E0061, argument #4 of type `&SaveItemImplementation<'_>` is missing |
| a route answering a response without saying whether what the request wrote is kept | E0308, expected `Outcome`, found `Response` |

A `match` over a behavior's answer is checked in the same way. The answer is an enum of exactly the
cases the model says it can answer, so a case added to the model stops this crate compiling until a
route says what the case is answered with, and whether what was written on the way to it is kept.

## How a request is served

A run is a mark on an arena that belongs to one thread. An async handler may resume on another
thread after an `.await`, so a run cannot be held across one, and the closure a run is opened with is
synchronous. Each handler hands its work to `App::handle`, which runs it with
`tokio::task::spawn_blocking` on a thread of its own: one run of the library and one SQLite
transaction, with the three behaviors bound inside the transaction.

```rust
let load_cart = LoadCartImplementation::new(library, SqlLoadCart(&transaction));
// ...
let behaviors = Behaviors {
    add_item_to_cart: AddItemToCart::bind(library, &load_product, &load_cart, &save_item),
    // ...
};
library.run(|run| work(&behaviors, run, &transaction))
```

An implementation borrows the transaction, and a bound behavior borrows the implementations, so
none of them can be called once the transaction is committed or rolled back. Binding for each
request costs a box for each implementation.

What comes back out of the run is an `Outcome`: the response, whose body is text by then, and
whether what the request wrote is kept. That the domain answered is not that its answer is to be
kept. `loadCart` makes a new user's cart row before the capacity is decided, and a command the model
then refuses (`CartFull`) must not leave that row behind. So every arm of every route says which,
`Commit` for the answer a command succeeds with and `Rollback` for every refusal, and there is no
conversion from a `Response` that would commit by default. A request that ends without an answer,
where the run ends for a reason the library numbers or an implementation fails, is rolled back and
answered with a 500. A panic in an implementation does not unwind through the library: the binding
catches it, and raises it again where the call into the library returns, which ends the blocking
task, rolls the transaction back, and is answered with a 500 as well.

The database is one connection behind a `Mutex`. SQLite writes one transaction at a time whatever
the application does, and a pool would not change what this example shows.

## How a request is read

Two parties read a request, and each owns a different part of what it means. The model owns what a
value is: which fields a type has, which case an orderer is, and every rule a type states, a
positive quantity, a name that is not blank and no longer than 100, a corporate number of thirteen
digits. None of that is written again in Rust. The boundary owns how a client writes a value: an id
is a UUID in lower case, an email is trimmed, lowercased and shaped like one, a name is trimmed.
Each of those is a raoh decoder, which writes the value in its form and refuses what cannot be
written so, as one step. Trimming a name is not a rule the model could state instead: an invariant
decides whether a value holds and never rewrites it, so a model asked to trim would keep
`"  Taro  "` as it came.

Where the boundary owns the value the model reads, an id, the two decoders are piped: the model
reads what the boundary answered, and nothing where it refused. Where the model reads a value whole
and the boundary owns some of its members, an orderer's email and names, a `pipe` would stop at the
first refusal, and an orderer whose email is refused would never reach the model, which alone can
say that a corporation has no company name. So each member is decoded on its own, and the model
reads the value whichever of them was refused (`http::boundary::members`):

```rust
field("userId", model.after(uuid(), |run, id: &String| UserId::new(run, id))),
field("orderer", model.members(
    vec![
        ("email", text(string().trim().lowercase().email())),
        ("name", text(string().trim())),
        ("companyName", text(string().trim())),
    ],
    |run, it| Orderer::decode(run, &it.to_string()),
)),
```

What the model is handed of a member is what the boundary's decoder answered for it, and of a
refused member nothing. The model never reads text the boundary refused, so what it sees does not
depend on whether the rest of the request was valid: a rule relating two members sees decoded
values or no value, never a raw one. It reports a refused member as missing, which is only that it
was taken out, and that one issue, at the member's own path, is dropped; nothing inside the member
is the model's to report, since it was not handed it. A corporation whose email is not shaped like
one and whose company name and corporate number are missing is one 400 with three issues: raoh
found the first, and the model the other two.

```json
{"issues": [
  {"path": "/orderer/email", "code": "invalid_format", ...},
  {"path": "/orderer/companyName", "code": "missing_field", ...},
  {"path": "/orderer/corporateNumber", "code": "missing_field", ...}]}
```

A rule a type states is reported at the field's path. A newtype's rule with a Raoh equivalent is
reported under that constraint's code and message key (`too_long`, `invalid_format`), as the JVM
reports it; any other rule is `invariant_violation`, with the type and its module in the issue's
metadata.

The database implementations read a row back through the model's decoder too, handed the row as
JSON under the type's field names, so what the database holds is checked by the model where it meets
it.

## Building and running it

What it needs is what the repository's own build needs: Maven, Cargo, and a C compiler, for the
SQLite rusqlite builds.

    bin/build
    cargo test

`bin/build` runs the command line at the root of the clone:

    mvn -q process-classes exec:java \
        -Dargs='--library <here>/build/native --rust <here>/build/rust --crate model examples/cart-model'

It writes the library into `build/native` and its binding into `build/rust`, as the crate `model`,
which `Cargo.toml` depends on by path. The module is `com.example.cart.domain`, so its types are
`model::com::example::cart::domain::*`, under the names the model spells them with
(`item.productId()`). The runtime the binding calls comes from `bindings/rust/runtime`, patched in
until it is published.

The rows of the five injected behaviors state what an implementation should answer and are owed by
one; nothing in the model can answer them.

To serve it:

    cargo run

The database is `build/cart.sqlite` unless `CART_DATABASE` names another file. It is seeded with the
user `11111111-1111-1111-1111-111111111111`, a product on sale at 1200
(`33333333-3333-3333-3333-333333333333`) and one no longer on sale
(`44444444-4444-4444-4444-444444444444`).

    curl -X POST localhost:8080/carts/items \
        -d '{"userId":"11111111-1111-1111-1111-111111111111","productId":"33333333-3333-3333-3333-333333333333","quantity":5}'
    curl -X POST localhost:8080/carts/checkout \
        -d '{"userId":"11111111-1111-1111-1111-111111111111","orderer":{"type":"Individual","email":"taro@example.com","name":"Taro"}}'

The second answers the order as the model writes it:

    {"id":"…","userId":"11111111-…","orderer":{"type":"Individual","email":"taro@example.com","name":"Taro"},
     "lines":[{"productId":"33333333-…","quantity":5,"unitPrice":1200}],
     "charge":{"subtotal":6000,"discount":600,"total":5400}}

The routes are every host's: `POST /carts/items`, `POST /carts/checkout`, `POST /carts/quote` and
`GET /carts/items?userId=...`. A body that does not decode is a 400 with raoh's issues, each with its
path. A business case the model answers (`CartFull`, `SaleEnded`, `EmptyCart`, `ProductNotFound`) is
a 422. The listing of a cart is the one route the model plays no part in: the model has no behavior
for a screen's listing and needs none, so the handler reads the rows and writes them out as they are.

## What reads differently in Rust

An answer no declaration names, such as `Product | ProductNotFound`, is an enum named after its
members in the manifest's order (`ProductOrProductNotFound`), which the routes import under a short
name (`Placed`, `Quoted`). A behavior's answer is built by naming the variant:
`ProductOrProductNotFound::ProductNotFound(...)`.

A constructor answers a `Construction`, the value or the issue it was refused with, inside a
`Result` whose error is the run ending. An implementation treats a refused value as its own failure
(`db::made`); a route never builds a value that can be refused except an id made from a UUID, which
is `unreachable!` where it is refused.

A value of the model is a `Copy` handle with the lifetime of its run (`Order<'run>`), and reading a
field copies what it holds into Rust's own memory (`String`, `i64`, `Vec`). Nothing the application
keeps across requests, the library and the connection, holds a value of the model.
