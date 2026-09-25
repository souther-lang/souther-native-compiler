# raoh-php-cart

The PHP counterpart of
[`boundaries-not-layers/examples/raoh-souther`](https://github.com/kawasima/boundaries-not-layers/tree/main/examples/raoh-souther).
The domain is the same `cart.sou`, compiled by this repository into a shared library and its PHP
binding. What is written in PHP is the boundaries around it: HTTP JSON decoded with
[raoh-php](https://github.com/kawasima/raoh-php) into the model's values, and the behaviors the
model asks a host for implemented over PDO and SQLite.

## What is in it

The rules of the cart are in `model/cart.sou` and nowhere else: the capacity of 10000, which a
`PendingItem` holds or is not built; the 10% discount at 5000 and above; that an empty cart is not
ordered and a quotation is for a corporation. Its `example` rows state what each behavior answers,
and they run when it is built, so a rule that stopped holding stops the build.

The PHP is two directories, one for each boundary. `src/Http` decodes a request into the model's
values, applies a behavior, and picks the response with a `match` on the class of what it answered.
`src/Database` holds the five behaviors the model leaves to a host (`loadProduct`, `loadCart`,
`saveItem`, `priceCart`, `saveOrder`), each a class extending the one the binding generates for it.
`src/CartApplication.php` binds the three composed behaviors (`addItemToCart`, `placeOrder`,
`issueQuote`) to those once, and routes the requests.

A request is decoded in two steps, as in the Java example. raoh-php checks the form of each field
and normalises it: a UUID, a positive quantity, an email trimmed and lowercased, a corporate number
of thirteen digits. What it hands on is read by the decoder the binding generates for the type,
`UserId::decoder($session)` where the Java example calls `UserId.decoder()`, and that decoder is the
model's: it knows which fields a type has, what the type states, and which case an orderer is.
None of that is written again in PHP.

There is no entity, no DTO, no repository and no view model. The classes of the model's types are
the binding's, and what a request is decoded into is a value of one of them. What an order or a
quotation is written back as is the model's own encoding of it, `encode()`, so there is no second
description of an order to keep in step with the first. A `match` over an answer lists the cases
the model says it can answer, and the binding types the answer as a union of exactly those classes.

The one route the model plays no part in is the listing of a cart, `GET /carts/items`. The model has
no behavior for a screen's listing and needs none, so the handler reads the rows and writes them out
as they are.

## Building and running it

What it needs is what the repository's own build needs: Maven, Cargo, Composer, and PHP 8.2 or later
with the `ffi`, `intl` and `pdo_sqlite` extensions.

    bin/build
    composer install
    vendor/bin/phpunit

`bin/build` runs the command line from #57 at the root of the clone:

    mvn -q process-classes exec:java \
        -Dargs='--library <here>/build/native --php <here>/build/php --namespace Model <here>/model'

It writes the library into `build/native` and its binding into `build/php`, under the namespace
`Model`, which `composer.json` maps as it maps the application's own classes. The module is
`com.example.cart.domain`, so its classes are `Model\Com\Example\Cart\Domain\*`. The runtime the
binding calls comes from `bindings/php/runtime` as a Composer path repository.

The rows of the five injected behaviors state what an implementation should answer and are owed by
one; nothing in the model can answer them.

To serve it:

    php -d ffi.enable=1 -S localhost:8080 -t public public/index.php

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

The routes are the Java example's: `POST /carts/items`, `POST /carts/checkout`, `POST /carts/quote`
and `GET /carts/items?userId=...`. A body that does not decode is a 400 with raoh-php's issues, each
with its path. A business case the model answers (`CartFull`, `SaleEnded`, `EmptyCart`,
`ProductNotFound`) is a 422.

## What differs from the Java example

Most of what differs follows from where a value of the model lives. On the JVM it is an object like
any other. Here it is held in the library's arena for the length of one run, `Binding::run`, and a
value used after its run has ended throws `Expired`. A value leaves a run only as its external form.

So each request is one run. The controller opens it, and everything that makes or reads a value of
the model happens inside: decoding the body, applying the behavior, and encoding the answer, which is
JSON text by the time the run ends. The decoders are made for the session they build values in
(`Decoders::addItem($session)`), where the Java ones are constants. Nothing the application keeps
across requests, the bound behaviors and the PDO implementations, holds a value of the model.

The Java example writes its responses by hand, from each value's accessors into a map. Here the
responses are the model's encoding, so an order's amounts are under `charge` and a line carries no
subtotal of its own. An orderer's `type` is the name of its case, `Individual` or `Corporation`, in
the request as in the response, where the Java example spells it in lower case. The Java example
also reads the cart's listing into `CartItem` values through a repository; here it is rows, for the
reason above.

The model is the Java example's `cart.sou` with two changes. The module has an `exposing` line,
which the Java example's does not. A module with no `exposing` clause publishes everything, and the
JVM backend reads it that way, but the checked program this repository reads answers that such a
module publishes nothing (souther-lang/souther#1959). The PHP binding writes what the module
publishes, so without the line it held no class for any type or behavior. Until that is fixed the
line lists the types and the three composed behaviors. The injected behaviors get their classes
without being listed, since the library asks a host for them. The other change is the discount, which
is `sub * 10 / 100` in the source. Since `/` answers the exact quotient, a `Rational`, the model says
it as `Int.truncatingDivide(sub * 10, 100)` and matches its `DivisionByZero` case, which the divisor
of 100 never takes.

The answers keep the unnamed unions the model writes, such as `Product | ProductNotFound`. The PHP
binding types them as a union of case classes, so there is no `LoadProductResult` as there is in Java,
and an implementation answers `ProductNotFound::of($session)` rather than calling a factory on its
base class. A `match` over an answer's class is not checked for the cases it leaves out until it runs,
where it throws `UnhandledMatchError`; the Java `switch` over a sealed type is checked when it is
compiled.

On the way in, the Java example tells an orderer's case apart with raoh's `discriminate` and a
decoder for each case. Here raoh-php checks whichever of the orderer's fields are present, and the
orderer as a whole goes to `OrdererCodec::decoder($session)`, which reads its `type` and the fields
that case has, as the model's encoding of an orderer says. A type's generated decoder is chained
with raoh-php's `pipe`, where the Java one is reached with `flatMap`. The database implementations
read a row back through the same decoder, handed the row as an array keyed by the type's field
names, as the Java ones hand the generated `decoder()` a map.

The rest is the platform. SQLite in place of H2, with UUIDs as text. PDO in place of jOOQ, and a
small `Transaction` in place of Spring's `TransactionTemplate`. Each test in
`tests/CartIntegrationTest.php` starts from a database of its own, where the Java tests share one.
