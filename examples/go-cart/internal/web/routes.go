// Package web is the HTTP boundary. A body is read into the arguments of a behavior, the behavior
// is called, and a type switch over what it answered picks the response. What an order or a
// quotation is written as is the model's own encoding of it.
//
// What a behavior answers is an interface with a type for each case the model says it can answer,
// and each route says for every one of them what it is answered with, and whether what was written
// on the way to it is kept: every case is a [cart.Outcome].
package web

import (
	"database/sql"
	"errors"
	"fmt"
	"net/http"
	"time"

	"github.com/google/uuid"
	"github.com/raoh-project/raoh-go"
	"github.com/raoh-project/raoh-go/encode"

	"example.com/go-cart/internal/cart"
	"example.com/go-cart/model"
	"example.com/go-cart/model/com/example/cart/domain"
)

// maxBody is the most a request body may be.
const maxBody = 1 << 20

// Router is the routes of the cart over app.
func Router(app *cart.App) http.Handler {
	mux := http.NewServeMux()
	mux.Handle("POST /carts/items", route(app, addItem))
	mux.Handle("GET /carts/items", route(app, listItems))
	mux.Handle("POST /carts/checkout", route(app, checkout))
	mux.Handle("POST /carts/quote", route(app, quote))
	return mux
}

// handler is what a route does in its request's run.
type handler func(b cart.Behaviors, r *model.Run, req *http.Request, tx *sql.Tx) (cart.Outcome, error)

func route(app *cart.App, h handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, req *http.Request) {
		app.Handle(req.Context(), func(b cart.Behaviors, r *model.Run, tx *sql.Tx) (cart.Outcome, error) {
			return h(b, r, req, tx)
		}).Write(w)
	})
}

// addItem is POST /carts/items.
func addItem(b cart.Behaviors, r *model.Run, req *http.Request, _ *sql.Tx) (cart.Outcome, error) {
	args, err := decode(req, addItemRequest(r))
	if err != nil {
		return cart.Outcome{}, err
	}

	answer, err := b.AddItemToCart.Call(r, args.userID, args.productID, args.quantity)
	if err != nil {
		return cart.Outcome{}, err
	}
	switch answer.(type) {
	case domain.ItemAdded:
		return cart.Commit(cart.Created(nil)), nil
	case domain.ProductNotFound:
		return refused("product_not_found")
	case domain.SaleEnded:
		return refused("sale_ended")
	case domain.CartFull:
		return refused("cart_full")
	}
	return unanswered("addItemToCart", answer)
}

// checkout is POST /carts/checkout.
func checkout(b cart.Behaviors, r *model.Run, req *http.Request, _ *sql.Tx) (cart.Outcome, error) {
	args, err := decode(req, checkoutRequest(r))
	if err != nil {
		return cart.Outcome{}, err
	}
	// An id this host made that the model refuses is the host's fault, and a 500.
	orderID, err := domain.NewOrderId(r, uuid.NewString())
	if err != nil {
		return cart.Outcome{}, fmt.Errorf("the model refused an order id this host made: %w", err)
	}

	answer, err := b.PlaceOrder.Call(r, orderID, args.userID, args.orderer)
	if err != nil {
		return cart.Outcome{}, err
	}
	switch answer := answer.(type) {
	case domain.OrderPlaced:
		return cart.Commit(cart.Created([]byte(answer.Order().Encode()))), nil
	case domain.EmptyCart:
		return refused("empty_cart")
	case domain.SaleEnded:
		return refused("sale_ended")
	case domain.ProductNotFound:
		return refused("product_not_found")
	}
	return unanswered("placeOrder", answer)
}

// quote is POST /carts/quote, for a corporation only.
func quote(b cart.Behaviors, r *model.Run, req *http.Request, _ *sql.Tx) (cart.Outcome, error) {
	args, err := decode(req, checkoutRequest(r))
	if err != nil {
		return cart.Outcome{}, err
	}
	// issueQuote takes a Corporation, so the orderer is narrowed here, over both of its cases.
	var corporation domain.Corporation
	switch orderer := args.orderer.Case().(type) {
	case domain.Corporation:
		corporation = orderer
	case domain.Individual:
		return refused("quote_for_corporations_only")
	}
	quoteID, err := domain.NewQuoteId(r, uuid.NewString())
	if err != nil {
		return cart.Outcome{}, fmt.Errorf("the model refused a quote id this host made: %w", err)
	}
	validUntil := encode.Date().Encode(time.Now().AddDate(0, 0, 30))

	answer, err := b.IssueQuote.Call(r, quoteID, args.userID, corporation, validUntil)
	if err != nil {
		return cart.Outcome{}, err
	}
	switch answer := answer.(type) {
	case domain.Quotation:
		return cart.Commit(cart.OK([]byte(answer.Encode()))), nil
	case domain.EmptyCart:
		return refused("empty_cart")
	case domain.SaleEnded:
		return refused("sale_ended")
	case domain.ProductNotFound:
		return refused("product_not_found")
	}
	return unanswered("issueQuote", answer)
}

// decode is req's body read by d. A body that is not JSON, not what d reads, or longer than a body
// may be is the client's, a [*cart.ClientError]; any other error is not.
func decode[T any](req *http.Request, d raoh.Decoder[any, T]) (T, error) {
	value, err := raoh.DecodeJSONFrom(req.Body, maxBody, d)
	return value, clientError(err)
}

// clientError is err as the client's where it is the issues found in what the client sent, or a body
// too long, and err as it is otherwise.
func clientError(err error) error {
	if _, ok := errors.AsType[*raoh.Issues](err); ok || errors.Is(err, raoh.ErrInputTooLarge) {
		return &cart.ClientError{Err: err}
	}
	return err
}

// refused is a business case the model answered, by name. It keeps nothing.
func refused(name string) (cart.Outcome, error) {
	return cart.Rollback(cart.Unprocessable(name)), nil
}

// unanswered is a case a behavior answered that its route has no answer for. Go does not check
// that a type switch has a case for each type of an interface, so a case added to the model comes
// here until a route says what it is answered with.
func unanswered(behavior string, answer any) (cart.Outcome, error) {
	return cart.Outcome{}, fmt.Errorf("%s answered %T, which its route does not answer", behavior, answer)
}
