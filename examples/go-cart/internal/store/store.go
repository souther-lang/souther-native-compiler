// Package store implements over SQLite the five behaviors the model leaves to a host (loadProduct,
// loadCart, saveItem, priceCart, saveOrder), each a struct holding the transaction of the request
// it answers in and implementing the interface the binding generates for the behavior.
//
// A row meets the model where it is read: each is written by an encoder in the form the model's
// type is written in, under the type's field names, and read by that type's decoder, so the model
// checks what the database holds as it checks what a client sends. What the model refuses there is
// the implementation's own failure: a row the model will not take is not an answer.
package store

import (
	"database/sql"
	_ "embed"
	"encoding/json"

	"github.com/raoh-project/raoh-go/encode"

	"example.com/go-cart/model"
	"example.com/go-cart/model/com/example/cart/domain"
)

var (
	//go:embed schema.sql
	schema string
	//go:embed data.sql
	seed string
)

// Prepare gives db the schema and seeds it with the demo user and products.
func Prepare(db *sql.DB) error {
	if _, err := db.Exec(schema); err != nil {
		return err
	}
	_, err := db.Exec(seed)
	return err
}

// productRow is a row of product.
type productRow struct {
	id     string
	onSale bool
	price  int64
}

// cartRow is a user's cart row, with the total quantity of what is in it.
type cartRow struct {
	id       string
	quantity int64
}

// lineRow is a line of a cart with the price of its product.
type lineRow struct {
	productID string
	quantity  int64
	unitPrice int64
}

// The forms of com.example.cart.domain the rows are written in.
var (
	productForm = encode.Object(
		encode.Property("id", func(p productRow) string { return p.id }, encode.String()),
		encode.Property("onSale", func(p productRow) bool { return p.onSale }, encode.Bool()),
		encode.Property("price", func(p productRow) int64 { return p.price }, encode.Int64()),
	)
	cartForm = encode.Object(
		encode.Property("id", func(c cartRow) string { return c.id }, encode.String()),
		encode.Property("currentQuantity", func(c cartRow) int64 { return c.quantity }, encode.Int64()),
	)
	lineForm = encode.Object(
		encode.Property("productId", func(l lineRow) string { return l.productID }, encode.String()),
		encode.Property("quantity", func(l lineRow) int64 { return l.quantity }, encode.Int64()),
		encode.Property("unitPrice", func(l lineRow) int64 { return l.unitPrice }, encode.Int64()),
	)
	pricedCartForm = encode.Object(
		encode.Property("lines", func(lines []lineRow) []lineRow { return lines }, encode.List(lineForm)),
	)
)

// read is row, written by form, as the model's decode reads it.
func read[R, T any](r *model.Run, form encode.Encoder[R, map[string]any], row R,
	decode func(*model.Run, []byte) (T, error)) (T, error) {
	text, err := json.Marshal(form.Encode(row))
	if err != nil {
		var zero T
		return zero, err
	}
	return decode(r, text)
}

// Assert that each implements the behavior the binding generates for it.
var (
	_ domain.LoadProduct = LoadProduct{}
	_ domain.LoadCart    = LoadCart{}
	_ domain.SaveItem    = SaveItem{}
	_ domain.PriceCart   = PriceCart{}
	_ domain.SaveOrder   = SaveOrder{}
)
