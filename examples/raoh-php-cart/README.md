# raoh-php-cart

The PHP counterpart of
[`boundaries-not-layers/examples/raoh-souther`](https://github.com/kawasima/boundaries-not-layers/tree/main/examples/raoh-souther).
The domain is the same `cart.sou`, compiled by this repository into a shared library and its PHP
binding. The boundaries are PHP: HTTP JSON decoded with [raoh-php](https://github.com/kawasima/raoh-php)
into the model's values, the behaviors the model asks a host for implemented over PDO and SQLite,
and the answers written back as JSON.

The PHP reads like the Java it is a port of. Each injected behavior (`loadProduct`, `loadCart`,
`saveItem`, `priceCart`, `saveOrder`) is a class extending the one the binding generates for it, in
`src/Infrastructure`. The composed behaviors (`addItemToCart`, `placeOrder`, `issueQuote`) are bound
to those once, in `src/CartApplication.php`, which is what `CartConfig` does with Spring. The
controller applies a composed behavior and picks the response with a `match` on the class of what it
answered, where the Java `switch`es on it.

The router is a few lines of plain PHP and not a framework, so what the example shows is the binding
and not a container.

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

The rows of `cart.sou` run as part of that build. A row that does not hold refuses the build, so
there is no separate step for them. The rows of the five injected behaviors state what an
implementation should answer and are owed by one; nothing in the model can answer them.

To serve it:

    php -d ffi.enable=1 -S localhost:8080 -t public public/index.php

The database is `build/cart.sqlite` unless `CART_DATABASE` names another file. It is seeded with the
user `11111111-1111-1111-1111-111111111111`, a product on sale at 1200
(`33333333-3333-3333-3333-333333333333`) and one no longer on sale
(`44444444-4444-4444-4444-444444444444`).

    curl -X POST localhost:8080/carts/items \
        -d '{"userId":"11111111-1111-1111-1111-111111111111","productId":"33333333-3333-3333-3333-333333333333","quantity":5}'
    curl -X POST localhost:8080/carts/checkout \
        -d '{"userId":"11111111-1111-1111-1111-111111111111","orderer":{"type":"individual","email":"taro@example.com","name":"Taro"}}'

The routes are the Java example's: `POST /carts/items`, `POST /carts/checkout`, `POST /carts/quote`
and `GET /carts/items?userId=...`. A body that does not decode is a 400 with raoh-php's issues, each
with its path. A business case the model answers (`CartFull`, `SaleEnded`, `EmptyCart`,
`ProductNotFound`) is a 422.

## What differs from the Java example

Most of what differs follows from where a value of the model lives. On the JVM it is an object like
any other. Here it is held in the library's arena for the length of one run, `Binding::run`, and a
value used after its run has ended throws `Expired`. A value leaves a run only as its external form.

So each request is one run. The controller opens it, and everything that makes or reads a value of
the model happens inside: decoding the body, applying the behavior, and reading the answer into the
response body, which is plain PHP arrays by the time the run ends. The decoders are made for the
session they build values in (`JsonCartDecoders::addItem($session)`), where the Java ones are
constants. Nothing holds a value of the model across requests. What the application does keep, the
bound behaviors and the PDO implementations, holds no value of the model at all.

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

On the way in, the web boundary builds values with each type's `of`, which takes the typed values
raoh-php decoded and answers a `Raoh\Result`, so the model's `invariant_violation` lands under the
field's path like any other issue. The gateways read a database row back with the type's `decode`,
from the row laid out in the type's external form, which is what the Java gateways do with a map and
the generated `decoder()`. `saveOrder` answers `OrderPlaced::of($session, $order)` where the Java
encodes the order and decodes it again.

The rest is the platform. SQLite in place of H2, with UUIDs as text. PDO in place of jOOQ, and a
small `Transaction` in place of Spring's `TransactionTemplate`. Each test in
`tests/CartIntegrationTest.php` starts from a database of its own, where the Java tests share one.
