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
	"log/slog"
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
	args, err := raoh.DecodeJSONFrom(req.Body, maxBody, addItemRequest(r))
	if response, ok := refusal(err); ok {
		return cart.Rollback(response), nil
	} else if err != nil {
		return cart.Outcome{}, err
	}

	answer, err := b.AddItemToCart.Call(r, args.userID, args.productID, args.quantity)
	if err != nil {
		return cart.Outcome{}, err
	}
	switch answer.(type) {
	case domain.CartFullOrItemAddedOrProductNotFoundOrSaleEndedItemAdded:
		return cart.Commit(cart.Created(nil)), nil
	case domain.CartFullOrItemAddedOrProductNotFoundOrSaleEndedProductNotFound:
		return refused("product_not_found")
	case domain.CartFullOrItemAddedOrProductNotFoundOrSaleEndedSaleEnded:
		return refused("sale_ended")
	case domain.CartFullOrItemAddedOrProductNotFoundOrSaleEndedCartFull:
		return refused("cart_full")
	}
	return unanswered("addItemToCart", answer)
}

// checkout is POST /carts/checkout.
func checkout(b cart.Behaviors, r *model.Run, req *http.Request, _ *sql.Tx) (cart.Outcome, error) {
	args, err := raoh.DecodeJSONFrom(req.Body, maxBody, checkoutRequest(r))
	if response, ok := refusal(err); ok {
		return cart.Rollback(response), nil
	} else if err != nil {
		return cart.Outcome{}, err
	}
	orderID, err := domain.NewOrderId(r, uuid.NewString())
	if err != nil {
		return refusedWhatTheHostMade(err)
	}

	answer, err := b.PlaceOrder.Call(r, orderID, args.userID, args.orderer)
	if err != nil {
		return cart.Outcome{}, err
	}
	switch answer := answer.(type) {
	case domain.EmptyCartOrOrderPlacedOrProductNotFoundOrSaleEndedOrderPlaced:
		return cart.Commit(cart.Created([]byte(answer.Value.Order().Encode()))), nil
	case domain.EmptyCartOrOrderPlacedOrProductNotFoundOrSaleEndedEmptyCart:
		return refused("empty_cart")
	case domain.EmptyCartOrOrderPlacedOrProductNotFoundOrSaleEndedSaleEnded:
		return refused("sale_ended")
	case domain.EmptyCartOrOrderPlacedOrProductNotFoundOrSaleEndedProductNotFound:
		return refused("product_not_found")
	}
	return unanswered("placeOrder", answer)
}

// quote is POST /carts/quote, for a corporation only.
func quote(b cart.Behaviors, r *model.Run, req *http.Request, _ *sql.Tx) (cart.Outcome, error) {
	args, err := raoh.DecodeJSONFrom(req.Body, maxBody, checkoutRequest(r))
	if response, ok := refusal(err); ok {
		return cart.Rollback(response), nil
	} else if err != nil {
		return cart.Outcome{}, err
	}
	// issueQuote takes a Corporation, so the orderer is narrowed here, over both of its cases.
	var corporation domain.Corporation
	switch orderer := args.orderer.Case().(type) {
	case domain.OrdererCorporation:
		corporation = orderer.Value
	case domain.OrdererIndividual:
		return refused("quote_for_corporations_only")
	}
	quoteID, err := domain.NewQuoteId(r, uuid.NewString())
	if err != nil {
		return refusedWhatTheHostMade(err)
	}
	validUntil := encode.Date().Encode(time.Now().AddDate(0, 0, 30))

	answer, err := b.IssueQuote.Call(r, quoteID, args.userID, corporation, validUntil)
	if err != nil {
		return cart.Outcome{}, err
	}
	switch answer := answer.(type) {
	case domain.EmptyCartOrProductNotFoundOrQuotationOrSaleEndedQuotation:
		return cart.Commit(cart.OK([]byte(answer.Value.Encode()))), nil
	case domain.EmptyCartOrProductNotFoundOrQuotationOrSaleEndedEmptyCart:
		return refused("empty_cart")
	case domain.EmptyCartOrProductNotFoundOrQuotationOrSaleEndedSaleEnded:
		return refused("sale_ended")
	case domain.EmptyCartOrProductNotFoundOrQuotationOrSaleEndedProductNotFound:
		return refused("product_not_found")
	}
	return unanswered("issueQuote", answer)
}

// refusal is the response a body that was not read comes to: a 400 with the issues found in it,
// or a 413 where it is longer than a body may be. Anything else is no refusal of the client's.
func refusal(err error) (cart.Response, bool) {
	if issues, ok := errors.AsType[*raoh.Issues](err); ok {
		return cart.BadRequest(issues), true
	}
	if errors.Is(err, raoh.ErrInputTooLarge) {
		return cart.TooLarge(), true
	}
	return cart.Response{}, false
}

// refused is a business case the model answered, by name. It keeps nothing.
func refused(name string) (cart.Outcome, error) {
	return cart.Rollback(cart.Unprocessable(name)), nil
}

// refusedWhatTheHostMade is what a request comes to where the model refused a value this host made
// on its own, such as an id: the fault is the host's, not the client's, so it is a 500 and nothing
// is kept. Which values the model admits is the model's to say, and nothing here assumes the answer.
func refusedWhatTheHostMade(err error) (cart.Outcome, error) {
	if _, ok := errors.AsType[*raoh.Issues](err); !ok {
		return cart.Outcome{}, err
	}
	slog.Error("the model refused a value the host made", "issues", err)
	return cart.Rollback(cart.Internal()), nil
}

// unanswered is a case a behavior answered that its route has no answer for. Go does not check
// that a type switch has a case for each type of an interface, so a case added to the model comes
// here until a route says what it is answered with.
func unanswered(behavior string, answer any) (cart.Outcome, error) {
	return cart.Outcome{}, fmt.Errorf("%s answered %T, which its route does not answer", behavior, answer)
}
