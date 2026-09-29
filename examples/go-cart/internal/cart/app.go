// Package cart is the wiring: the behaviors the model leaves to a host implemented over SQLite, and
// the behaviors the routes call bound to them, in one run of the library and one transaction for
// each request.
package cart

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"log/slog"

	"example.com/go-cart/internal/store"
	"example.com/go-cart/model"
	"example.com/go-cart/model/com/example/cart/domain"
)

// App is the library and the database, shared by every request.
type App struct {
	library *model.Library
	db      *sql.DB
}

// Behaviors are the three behaviors the routes call, bound for one request.
type Behaviors struct {
	AddItemToCart domain.AddItemToCartBound
	PlaceOrder    domain.PlaceOrderBound
	IssueQuote    domain.IssueQuoteBound
}

// Work is what a route does in its request's run: it reads the request, calls a behavior, and says
// what the request comes to. tx is the request's transaction, for a route that reads the database
// itself.
type Work func(b Behaviors, r *model.Run, tx *sql.Tx) (Outcome, error)

// New is the application over library and db, which is given the schema and seeded with the demo
// user and products.
//
// db is used one connection at a time. SQLite writes one transaction at a time whatever the
// application does, and an example has no need of a pool to say so.
func New(library *model.Library, db *sql.DB) (*App, error) {
	db.SetMaxOpenConns(1)
	if err := store.Install(db); err != nil {
		return nil, err
	}
	return &App{library: library, db: db}, nil
}

// Handle is the response work comes to, in one run of the library and one transaction, which is
// committed or rolled back as the [Outcome] says.
//
// A run is a mark on an arena that belongs to the OS thread it was opened on, and every value of
// the model made in it is good until it ends. library.Run holds the goroutine on its thread until
// then, so a handler needs no thread of its own: the request's goroutine is the run's. What comes
// back out of the run is a [Response], whose body is text by then.
//
// An error work returns is a 500, except a [*ClientError], which is the client's: a 400 with the
// issues found in the request, or a 413. Either way nothing is kept.
func (a *App) Handle(ctx context.Context, work Work) (response Response) {
	tx, err := a.db.BeginTx(ctx, nil)
	if err != nil {
		return failed(err)
	}
	// Whatever the request comes to, what it wrote is dropped unless it is committed below.
	defer tx.Rollback()
	// A panic in an implementation comes back out of the call that reached it, never through the
	// library, and ends here.
	defer func() {
		if p := recover(); p != nil {
			response = failed(fmt.Errorf("panic: %v", p))
		}
	}()

	var outcome Outcome
	err = a.library.Run(func(r *model.Run) error {
		// An implementation holds the transaction, and what stands for it is held until the run
		// ends, so none of them is called once the transaction is committed or rolled back.
		priceCart := domain.ImplementPriceCart(r, store.PriceCart{Tx: tx})
		b := Behaviors{
			AddItemToCart: domain.BindAddItemToCart(r,
				domain.ImplementLoadProduct(r, store.LoadProduct{Tx: tx}),
				domain.ImplementLoadCart(r, store.LoadCart{Tx: tx}),
				domain.ImplementSaveItem(r, store.SaveItem{Tx: tx})),
			PlaceOrder: domain.BindPlaceOrder(r, priceCart,
				domain.ImplementSaveOrder(r, store.SaveOrder{Tx: tx})),
			IssueQuote: domain.BindIssueQuote(r, priceCart),
		}
		var err error
		outcome, err = work(b, r, tx)
		return err
	})
	if client, ok := errors.AsType[*ClientError](err); ok {
		return client.response()
	}
	if err != nil {
		// The run ended for a reason the library numbers, an implementation failed, or the route did.
		return failed(err)
	}
	if outcome.keep {
		if err := tx.Commit(); err != nil {
			return failed(err)
		}
	}
	return outcome.response
}

func failed(reason error) Response {
	slog.Error("a request ended without an answer", "reason", reason)
	return Internal()
}
