package web

import (
	"database/sql"
	"encoding/json"
	"math"
	"net/http"

	"github.com/raoh-project/raoh-go/encode"

	"example.com/go-cart/internal/cart"
	"example.com/go-cart/model"
)

// listItems is GET /carts/items?userId=…&page=…&size=…
//
// A listing for a screen, which the model has no behavior for and needs none: the rows are read
// and written out as they are.
func listItems(_ cart.Behaviors, r *model.Run, req *http.Request, tx *sql.Tx) (cart.Outcome, error) {
	query := map[string]any{}
	for name, values := range req.URL.Query() {
		query[name] = values[0]
	}
	asked, err := listRequest(r).Decode(query)
	if response, ok := refusal(err); ok {
		return cart.Rollback(response), nil
	} else if err != nil {
		return cart.Outcome{}, err
	}

	found, err := list(tx, asked)
	if err != nil {
		return cart.Outcome{}, err
	}
	body, err := json.Marshal(listingForm.Encode(found))
	if err != nil {
		return cart.Outcome{}, err
	}
	// A listing writes nothing, so there is nothing to keep.
	return cart.Rollback(cart.OK(body)), nil
}

type listing struct {
	total, page, size int64
	items             []listed
}

type listed struct {
	productID string
	quantity  int64
}

var listingForm = encode.Object(
	encode.Property("total", func(l listing) int64 { return l.total }, encode.Int64()),
	encode.Property("page", func(l listing) int64 { return l.page }, encode.Int64()),
	encode.Property("size", func(l listing) int64 { return l.size }, encode.Int64()),
	encode.Property("items", func(l listing) []listed { return l.items }, encode.List(encode.Object(
		encode.Property("productId", func(i listed) string { return i.productID }, encode.String()),
		encode.Property("quantity", func(i listed) int64 { return i.quantity }, encode.Int64()),
	))),
)

func list(tx *sql.Tx, asked listQuery) (listing, error) {
	found := listing{page: asked.page, size: asked.size, items: []listed{}}
	err := tx.QueryRow(`
		SELECT COUNT(*) FROM cart_item ci JOIN cart c ON c.cart_id = ci.cart_id
		WHERE c.user_id = ?`, asked.userID).Scan(&found.total)
	if err != nil {
		return listing{}, err
	}
	rows, err := tx.Query(`
		SELECT ci.product_id, ci.quantity
		FROM cart_item ci JOIN cart c ON c.cart_id = ci.cart_id
		WHERE c.user_id = ?
		ORDER BY ci.product_id
		LIMIT ? OFFSET ?`, asked.userID, asked.size, offset(asked))
	if err != nil {
		return listing{}, err
	}
	defer rows.Close()
	for rows.Next() {
		var item listed
		if err := rows.Scan(&item.productID, &item.quantity); err != nil {
			return listing{}, err
		}
		found.items = append(found.items, item)
	}
	return found, rows.Err()
}

// offset is the number of items before the page asked for, or the most there can be where that is
// more than an int64 holds.
func offset(asked listQuery) int64 {
	if asked.page > math.MaxInt64/asked.size {
		return math.MaxInt64
	}
	return asked.page * asked.size
}
