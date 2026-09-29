package web

import (
	"github.com/raoh-project/raoh-go"
	"github.com/raoh-project/raoh-go/encode"

	"example.com/go-cart/model"
	"example.com/go-cart/model/com/example/cart/domain"
)

// Request bodies, read into the arguments of a behavior.
//
// Two parties read a request, and each owns a different part of what it means. The model owns what
// a value is: which case an orderer is, the fields each case has, and every rule a type states, a
// positive quantity, a name that is not blank and a corporate number of thirteen digits among them.
// Nothing here says any of that again. The boundary owns how a client writes a value: an id is a
// UUID in lower case, an email is trimmed, lowercased and shaped like one, a name is trimmed.
//
// A request has no type of its own in the model, since a behavior takes its arguments by place, so
// each argument is a field here and read by its type's decoder or constructor. Every field is read
// whichever of them fails, so a request answers every issue it has at once, the boundary's and the
// model's.
//
// The model's steps are made from the run they read in, so the decoders here are made for one
// request.

type addItemArgs struct {
	userID    domain.UserId
	productID domain.ProductId
	quantity  domain.Quantity
}

// addItemRequest reads {"userId":"…","productId":"…","quantity":n} as the arguments of addItemToCart.
func addItemRequest(r *model.Run) raoh.Decoder[any, addItemArgs] {
	return raoh.Object(raoh.Fields().
		Field("userId", id(r, domain.NewUserId)).
		Field("productId", id(r, domain.NewProductId)).
		Field("quantity", domain.QuantityDecoder(r)),
	).Map(func(u domain.UserId, p domain.ProductId, q domain.Quantity) addItemArgs {
		return addItemArgs{u, p, q}
	})
}

type checkoutArgs struct {
	userID  domain.UserId
	orderer domain.Orderer
}

// checkoutRequest reads {"userId":"…","orderer":{…}} as a user and an orderer.
func checkoutRequest(r *model.Run) raoh.Decoder[any, checkoutArgs] {
	return raoh.Object(raoh.Fields().
		Field("userId", id(r, domain.NewUserId)).
		Field("orderer", members([]member{
			text("email", raoh.String().Trim().ToLower().Email()),
			text("name", raoh.String().Trim()),
			text("companyName", raoh.String().Trim()),
		}, domain.OrdererDecoder(r))),
	).Map(func(u domain.UserId, o domain.Orderer) checkoutArgs { return checkoutArgs{u, o} })
}

type listQuery struct {
	userID     string
	page, size int64
}

// listRequest reads ?userId=…&page=…&size=… of a listing. Only the user is the model's, checked as
// every other input is; a page or a size that is not a number is the first page of twenty.
func listRequest(r *model.Run) raoh.Decoder[any, listQuery] {
	return raoh.Object(raoh.Fields().
		Field("userId", id(r, domain.NewUserId).Map(domain.UserId.Value)).
		Field("page", count(0, 0)).
		Field("size", count(1, 20)),
	).Map(func(user string, page, size int64) listQuery { return listQuery{user, page, size} })
}

// id is a UUID as this API writes one and as the database keeps it, in lower case with its
// hyphens, made into a value of the model by construct. Another notation of one (braced, a URN,
// without hyphens) is not how a client writes an id here.
func id[T any](r *model.Run, construct func(*model.Run, string) (T, error)) raoh.Decoder[any, T] {
	return raoh.String().UUID().Map(encode.UUID()).
		AndThen(func(id string) (T, error) { return construct(r, id) })
}

// count is a number in a query, otherwise where there is none or it is no number, and never under
// least.
func count(least, otherwise int64) raoh.Decoder[any, int64] {
	return raoh.String().ToLong().Fallback(otherwise).
		Map(func(n int64) int64 { return max(n, least) })
}
