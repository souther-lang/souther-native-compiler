# go-cart

The cart example every host binding of a Souther library is shown with, written as a Go
application. The domain is `examples/cart-model`, the model every host's cart runs, compiled by this
repository into a shared library and its Go binding. What is written in Go is the boundaries around
it: HTTP served by `net/http`, JSON decoded with [raoh-go](https://github.com/raoh-project/raoh-go)
into the model's values, and the behaviors the model asks a host for implemented over SQLite with
`database/sql` and [modernc.org/sqlite](https://modernc.org/sqlite).

## What is in it

The rules of the cart are in `examples/cart-model/cart.sou` and nowhere else: the capacity of 10000,
which a `PendingItem` holds or is not built; the 10% discount at 5000 and above; that an empty cart
is not ordered, and that a quotation is for a corporation and holds for thirty days. Its `example`
rows state what each behavior answers, and they run when it is built, so a rule that stopped holding
stops the build before Go is reached.

The Go is three packages under `internal`. `web` reads a request into a behavior's arguments with
raoh decoders, calls the behavior, and picks the response with a type switch over what it answered:

```go
args, err := decode(req, checkoutRequest(r))
// ...
answer, err := b.PlaceOrder.Call(r, orderID, args.userID, args.orderer)
// ...
switch answer := answer.(type) {
case domain.OrderPlaced:
	return cart.Commit(cart.Created([]byte(answer.Order().Encode()))), nil
case domain.EmptyCart:
	return refused("empty_cart")
// ...
}
```

`store` holds the five behaviors the model leaves to a host (`loadProduct`, `loadCart`, `saveItem`,
`priceCart`, `saveOrder`), each a struct implementing the interface the binding generates for it.
`cart` binds the three composed behaviors (`addItemToCart`, `placeOrder`, `issueQuote`) to those for
each request, and runs the request in one run of the library and one transaction.

There is no entity, no DTO with struct tags, no repository and no validation package. The types a
request is read into are the binding's, and what an order or a quotation is written back as is the
model's own encoding of it, `Encode()`, so there is no second description of an order to keep in
step with the first.

## What Go would need, and what the model says instead

In Go by hand, a rule is kept by convention. A `Quantity` is a struct with an unexported field and a
`NewQuantity` that returns an error, but its zero value can still be made anywhere, and every caller
has to remember to go through the constructor. A rule that relates two values, such as the capacity
of a cart, has no type to live in at all: it is an `if` in a service, an error value, a check in the
JSON handler that reports it at the right path, and a `switch` somewhere mapping that error to
`CartFull`. Four places that have to agree on one rule, and a test for each.

Here it is one line of the model:

```
data PendingItem = { cart: Cart, item: CartItem }
    invariant withinCapacity = cart.currentQuantity.value + item.quantity.value <= 10000
```

and `addItemToCart` turns that into its answer:

```
guard PendingItem { cart = c, item = CartItem { productId = productId, quantity = quantity } } as pending
    else | withinCapacity -> CartFull
```

The binding's `NewPendingItem` refuses the value with an `invariant_violation` issue, its
`DecodePendingItem` reports the same at the value's path, and the behavior answers `CartFull`. None
of that is written in Go. `saveItem` is handed a `PendingItem`, and a `PendingItem` exists only
where the capacity holds, so the SQL that saves it checks nothing again.

The same goes for what a request may hold. A name that is not blank and at most 100 code points, a
corporate number of thirteen digits, an orderer that is an individual or a corporation with the
fields each has: those are types of the model, and `domain.DecodeOrderer` reports every one of them
in raoh's form, with its JSON Pointer, the code and the message key raoh-go uses for the same
constraint. There is no struct tag, no `validate:"required,len=13"`, and nothing to translate from
one validation library's errors into another's.

The rules are checked without a database. `fake loadCart` and the other fakes in `cart.sou` stand
for the injected behaviors, and the `example` rows run `addItemToCart`, `placeOrder` and
`issueQuote` over them when `bin/build` compiles the model. Nothing here mocks an interface.

The model is the same on every host. `examples/rust-cart`, `examples/php-cart` and this one build
one file, and an order is written in the one encoding the model gives it, so a client does not know
which host answered. `internal/web/cart_test.go` holds the same HTTP contract as the Rust and PHP
tests, case for case.

## How a request is read

Two parties read a request, and each owns a different part of what it means. The model owns what a
value is: which fields a type has, which case an orderer is, and every rule a type states. The
boundary owns how a client writes a value: an id is a UUID in lower case, an email is trimmed,
lowercased and shaped like one, a name is trimmed. Each of those is a raoh decoder, which writes the
value in its form and refuses what cannot be written so, as one step. Trimming a name is not a rule
the model could state instead: an invariant decides whether a value holds and never rewrites it.

The binding offers each type's decoder as a raoh decoder, `domain.QuantityDecoder(r)`, and each
constructor answers `(T, error)` with a refused value's issues as a `*raoh.Issues`, which is exactly
what raoh's `AndThen` takes. So the model's steps compose with the boundary's as they stand, and a
request is one decoder:

```go
raoh.Object(raoh.Fields().
	Field("userId", id(r, domain.NewUserId)).
	Field("productId", id(r, domain.NewProductId)).
	Field("quantity", domain.QuantityDecoder(r)),
).Map(func(u domain.UserId, p domain.ProductId, q domain.Quantity) addItemArgs {
	return addItemArgs{u, p, q}
})
```

`id` is raoh's UUID decoder, its encoder writing the UUID back as this API writes one, and the
model's constructor after it: `raoh.String().UUID().Map(encode.UUID()).AndThen(...)`. An issue the
model finds is reported at the path of the field, beside the boundary's, and any other error the
model answers, the run ending, stops the decode and is not taken for the client's.

Where the model reads a value whole and the boundary owns some of its members, an orderer's email
and names, an `AndThen` would stop at the first refusal, and an orderer whose email is refused would
never reach the model, which alone can say that a corporation has no company name. So each member is
decoded on its own, and the model reads the value whichever of them was refused
(`members` in `internal/web/boundary.go`):

```go
Field("orderer", members([]member{
	text("email", raoh.String().Trim().ToLower().Email()),
	text("name", raoh.String().Trim()),
	text("companyName", raoh.String().Trim()),
}, domain.OrdererDecoder(r)))
```

What the model is handed of a member is what the boundary's decoder answered for it, and of a
refused member nothing, so what it sees does not depend on whether the rest of the request was
valid. It reports a refused member as missing, which is only that it was taken out, and that one
issue, at the member's own path, is dropped. A corporation whose email is not shaped like one and
whose company name and corporate number are missing is one 400 with three issues: raoh found the
first, and the model the other two.

```json
{"issues": [
  {"path": "/orderer/email", "code": "invalid_format", ...},
  {"path": "/orderer/companyName", "code": "missing_field", ...},
  {"path": "/orderer/corporateNumber", "code": "missing_field", ...}]}
```

## Where a row meets the model

The database implementations read a row back through the model's decoder too. Each row is written
by a raoh encoder in the form the model's type is written in, under the type's field names, and read
by that type's raoh decoder, so the model checks what the database holds as it checks what a client
sends, and the encoder and the decoder are the two sides of one form:

```go
productForm = encode.Object(
	encode.Property("id", func(p productRow) string { return p.id }, encode.String()),
	encode.Property("onSale", func(p productRow) bool { return p.onSale }, encode.Bool()),
	encode.Property("price", func(p productRow) int64 { return p.price }, encode.Int64()),
)
// ...
product, err := domain.ProductDecoder(r).Decode(productForm.Encode(row))
```

A row the model refuses is the implementation's own failure: it comes back out of the call that
reached the implementation as a `*souther.HostError`, and the request is a 500. The responses the
application writes of its own, the issues of a 400, the name of a refused command, a page of a
cart's items, are encoders as well.

## How a request is served

A run is a mark on an arena that belongs to one OS thread, and every value of the model made in it
is good until it ends. `library.Run` holds the goroutine on its thread from the mark to the reset
(`runtime.LockOSThread`), so a handler needs no thread of its own: the request's goroutine is the
run's. `App.Handle` opens one run and one SQLite transaction, and binds the three behaviors inside
it:

```go
priceCart := domain.ImplementPriceCart(r, store.PriceCart{Tx: tx})
b := Behaviors{
	AddItemToCart: domain.BindAddItemToCart(r,
		domain.ImplementLoadProduct(r, store.LoadProduct{Tx: tx}),
		domain.ImplementLoadCart(r, store.LoadCart{Tx: tx}),
		domain.ImplementSaveItem(r, store.SaveItem{Tx: tx})),
	// ...
}
```

What stands for an implementation is held until the run ends, and the run ends before the
transaction is committed or rolled back, so a callback from the library never reaches a transaction
that is over. A behavior bound without one of the behaviors it depends on does not compile: `Bind`
takes each of them, in order.

What comes back out of the run is an `Outcome`: the response, whose body is text by then, and
whether what the request wrote is kept. That the domain answered is not that its answer is to be
kept. `loadCart` makes a new user's cart row before the capacity is decided, and a command the model
then refuses (`CartFull`) must not leave that row behind. So every case of every route says which,
`Commit` for the answer a command succeeds with and `Rollback` for every refusal. A route answers an
`Outcome` and not a `Response`, so a route that forgets to say does not compile.

Anything else a route comes to is an `error`, returned as Go returns one, and `Handle` decides the
response in one place. A request the client got wrong is a `*cart.ClientError`, which `decode`
makes of the issues raoh and the model found in a body, and is a 400 with those issues, or a 413 for
a body over a mebibyte. Any other error, where the run ends for a reason the library numbers, an
implementation fails, or the model refuses an id the host made, is a 500. A panic in an
implementation does not unwind through the library: the binding catches it and raises it again
where the call into the library returns, and `Handle` answers a 500. The transaction is rolled back
by a `defer` unless the `Outcome` says to commit, so none of these keeps anything.

The database is used one connection at a time. SQLite writes one transaction at a time whatever the
application does, and a pool would not change what this example shows.

## What Go checks when it runs

What Rust refuses to compile, the Go runtime refuses where it happens. A value of the model lives in
an arena that belongs to the run it was made in, and the run ends with the request. Using it after,
or from another goroutine, panics with a `*souther.Misuse` at the first use: it is a fault of the
program and not a condition a handler answers. `internal/cart/misuse_test.go` holds each:

| The mistake | What happens | In Rust |
|---|---|---|
| a value kept after its request, as a cache would keep one | panic, `ErrExpired` | E0521 |
| a value handed to another goroutine | panic, `ErrRunOnAnotherGoroutine` | E0277 |
| a bound behavior kept past its request, and with it the transaction its implementations hold | panic, `ErrExpired` | E0505 |
| a behavior bound without one of the behaviors it depends on | does not compile | E0061 |
| a route answering a response without saying whether what the request wrote is kept | does not compile | E0308 |

A type switch is not checked for the cases it leaves out, as Rust's `match` is, and a switch that
forgets one goes on with what it did not set: `saveOrder` given an orderer of a case the model has
gained would write a row of neither case, with no type and no email, and commit it. So the binding
declares every interface a union or a sum's cases is a sum type (`//sumtype:decl`), and
`scripts/go-cart-example.sh` runs [go-check-sumtype](https://github.com/alecthomas/go-check-sumtype)
over the application with `-default-signifies-exhaustive=false`: a switch over one names every case,
a `default` notwithstanding, and a case added to the model stops the build at every switch that does
not answer it. What a program built without the check comes to at run time is a failure and a 500,
never a row of the wrong shape: `saveOrder`'s switch fails in its `default`, and a route's switch
over a behavior's answer returns from each case it names and fails after it, in `unanswered`. Where
a route asks only whether a value is one case, as the quote route asks whether the orderer is a
`Corporation`, it asks with a type assertion, which answers every case the model has or gains.

## Building and running it

What it needs is what the repository's own build needs: Maven, Go and a C compiler, for the binding
cgo builds. SQLite is Go here, so nothing else is installed.

    bin/build
    go run github.com/alecthomas/go-check-sumtype/cmd/go-check-sumtype@v0.5.0 \
        -default-signifies-exhaustive=false ./...
    go test ./...

`bin/build` runs the command line:

    scripts/souther-native --library <here>/build/native --go <here>/build/go \
        --package example.com/go-cart/model examples/cart-model

It writes the library into `build/native` and its binding into `build/go`, as the module
`example.com/go-cart/model`, which `go.mod` replaces with that directory. The module is
`com.example.cart.domain`, so its types are in the package
`example.com/go-cart/model/com/example/cart/domain`, under the names the model spells them with, the
first letter a capital (`item.ProductId()`). The runtime the binding calls comes from
`bindings/go/runtime`, replaced in until it is published.

The rows of the five injected behaviors state what an implementation should answer and are owed by
one; nothing in the model can answer them.

To serve it:

    go run .

It serves on `localhost:8080` (`-addr`), loads the library from `build/native` (`-library`), and
keeps the database in `build/cart.sqlite` unless `-db` or `CART_DATABASE` names another file, and
stops on an interrupt once the requests it is answering are answered. The database is seeded with the
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
path, and one longer than a mebibyte is a 413. A business case the model answers (`CartFull`,
`SaleEnded`, `EmptyCart`, `ProductNotFound`) is a 422. The listing of a cart is the one route the
model plays no part in: the model has no behavior for a screen's listing and needs none, so the
handler reads the rows and writes them out with an encoder.

## What reads differently in Go

An answer no declaration names, such as `Product | ProductNotFound`, is an interface named after its
members in the manifest's order (`ProductOrProductNotFound`), and each member of it is the model's
own type, `domain.Product` or `domain.ProductNotFound`. A route's `case` names the type the model
answered, and an implementation answers one as it is: `loadProduct` returns the `ProductNotFound`
it made. Go lets a package write a method only on its own types, so a member that is a primitive,
such as `Int` in `Int | Free`, is held by a type of the union's instead (`FreeOrIntInt{Value: n}`);
the cart has none.

A sum the model declares, `Orderer`, has `Case()`, answering the value as the type of its case,
`domain.Individual` or `domain.Corporation`. `issueQuote` takes a `Corporation`, so the quote route
asks whether the orderer is one before it calls it, and refuses anything else.

A `Date` is the runtime's `souther.Date`, held as its year, month and day. The quote route hands
`issueQuote` today, and the model answers the quotation valid thirty days from it.

A value of the model is a struct holding a handle and the run it was made in. Reading a field copies
what it holds into Go's own memory (`string`, `int64`, a slice), and nothing the application keeps
across requests, the library and the database, holds a value of the model.
