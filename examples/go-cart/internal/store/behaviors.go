package store

import (
	"database/sql"
	"errors"

	"github.com/google/uuid"

	"example.com/go-cart/model"
	"example.com/go-cart/model/com/example/cart/domain"
)

// LoadProduct is loadProduct. No row is the model's own ProductNotFound.
type LoadProduct struct{ Tx *sql.Tx }

func (s LoadProduct) Apply(r *model.Run, id domain.ProductId) (domain.ProductOrProductNotFound, error) {
	var row productRow
	err := s.Tx.QueryRow(`SELECT product_id, on_sale, price FROM product WHERE product_id = ?`, id.Value()).
		Scan(&row.id, &row.onSale, &row.price)
	if errors.Is(err, sql.ErrNoRows) {
		none, err := domain.NewProductNotFound(r)
		return domain.ProductOrProductNotFoundProductNotFound{Value: none}, err
	}
	if err != nil {
		return nil, err
	}
	product, err := read(r, productForm, row, domain.DecodeProduct)
	return domain.ProductOrProductNotFoundProduct{Value: product}, err
}

// LoadCart is loadCart. It makes sure the user has a cart row, then reads the cart with the total
// quantity of what is in it in one aggregate query: the items themselves are not loaded, and the
// total is what the capacity rule needs.
//
// Making the row is a write before the command has been decided: a Cart has an id, and the row is
// where a new user's cart gets one. A command the model refuses after it (CartFull) keeps nothing,
// since its route answers a Rollback.
type LoadCart struct{ Tx *sql.Tx }

func (s LoadCart) Apply(r *model.Run, userID domain.UserId) (domain.Cart, error) {
	user := userID.Value()
	if _, err := s.Tx.Exec(`INSERT OR IGNORE INTO cart (cart_id, user_id) VALUES (?, ?)`,
		uuid.NewString(), user); err != nil {
		return domain.Cart{}, err
	}
	var row cartRow
	err := s.Tx.QueryRow(`
		SELECT c.cart_id, COALESCE(SUM(ci.quantity), 0)
		FROM cart c LEFT JOIN cart_item ci ON ci.cart_id = c.cart_id
		WHERE c.user_id = ?
		GROUP BY c.cart_id`, user).Scan(&row.id, &row.quantity)
	if err != nil {
		return domain.Cart{}, err
	}
	return read(r, cartForm, row, domain.DecodeCart)
}

// SaveItem is saveItem. It takes the cart and the item out of the PendingItem the model built, adds
// the quantity to the row already there or inserts one, and answers what it wrote.
//
// A PendingItem exists only where the capacity holds, so nothing here checks it again.
type SaveItem struct{ Tx *sql.Tx }

func (s SaveItem) Apply(r *model.Run, pending domain.PendingItem) (domain.ItemAdded, error) {
	cartID := pending.Cart().Id().Value()
	item := pending.Item()
	productID, quantity := item.ProductId().Value(), item.Quantity().Value()

	updated, err := s.Tx.Exec(`UPDATE cart_item SET quantity = quantity + ? WHERE cart_id = ? AND product_id = ?`,
		quantity, cartID, productID)
	if err != nil {
		return domain.ItemAdded{}, err
	}
	if n, err := updated.RowsAffected(); err != nil {
		return domain.ItemAdded{}, err
	} else if n == 0 {
		if _, err := s.Tx.Exec(`INSERT INTO cart_item (cart_item_id, cart_id, product_id, quantity) VALUES (?, ?, ?, ?)`,
			uuid.NewString(), cartID, productID, quantity); err != nil {
			return domain.ItemAdded{}, err
		}
	}
	return domain.NewItemAdded(r, item.ProductId(), item.Quantity())
}

// PriceCart is priceCart. It reads every line of the cart with its product in one query, sees that
// each product is there and on sale, and answers the lines with their prices as a PricedCart. A
// product that is gone or no longer on sale ends it with the model's own case, the first such line
// in the order of the product ids deciding which.
//
// Deciding that for each line stays here, in the implementation: the model has no traverse, and a
// fold cannot call another injected behavior. The products are joined to the lines rather than
// asked for one by one, so a cart is one query however many lines it has.
type PriceCart struct{ Tx *sql.Tx }

func (s PriceCart) Apply(r *model.Run, userID domain.UserId) (domain.PricedCartOrProductNotFoundOrSaleEnded, error) {
	lines, ended, err := s.lines(userID.Value())
	switch {
	case err != nil:
		return nil, err
	case ended == productGone:
		none, err := domain.NewProductNotFound(r)
		return domain.PricedCartOrProductNotFoundOrSaleEndedProductNotFound{Value: none}, err
	case ended == saleOver:
		over, err := domain.NewSaleEnded(r)
		return domain.PricedCartOrProductNotFoundOrSaleEndedSaleEnded{Value: over}, err
	}
	priced, err := read(r, pricedCartForm, lines, domain.DecodePricedCart)
	return domain.PricedCartOrProductNotFoundOrSaleEndedPricedCart{Value: priced}, err
}

// unpriced is why a cart has no price, or that it has one.
type unpriced int

const (
	priced unpriced = iota
	productGone
	saleOver
)

// lines are the lines of the user's cart with their prices, or why the first line without one has
// none.
func (s PriceCart) lines(user string) ([]lineRow, unpriced, error) {
	rows, err := s.Tx.Query(`
		SELECT ci.product_id, ci.quantity, p.on_sale, p.price
		FROM cart_item ci
		JOIN cart c ON c.cart_id = ci.cart_id
		LEFT JOIN product p ON p.product_id = ci.product_id
		WHERE c.user_id = ?
		ORDER BY ci.product_id`, user)
	if err != nil {
		return nil, priced, err
	}
	defer rows.Close()
	var lines []lineRow
	for rows.Next() {
		var line lineRow
		var onSale sql.NullBool
		var price sql.NullInt64
		if err := rows.Scan(&line.productID, &line.quantity, &onSale, &price); err != nil {
			return nil, priced, err
		}
		if !onSale.Valid || !price.Valid {
			return nil, productGone, nil
		}
		if !onSale.Bool {
			return nil, saleOver, nil
		}
		line.unitPrice = price.Int64
		lines = append(lines, line)
	}
	return lines, priced, rows.Err()
}

// SaveOrder is saveOrder: one row for the order and one for each of its lines. The orderer is laid
// out flat by which case it is, since an individual and a corporation fill different columns.
type SaveOrder struct{ Tx *sql.Tx }

func (s SaveOrder) Apply(r *model.Run, order domain.Order) (domain.OrderPlaced, error) {
	var kind, email string
	var name, companyName, corporateNumber sql.NullString
	switch orderer := order.Orderer().Case().(type) {
	case domain.OrdererIndividual:
		kind, email = "individual", orderer.Value.Email().Value()
		name = present(orderer.Value.Name().Value())
	case domain.OrdererCorporation:
		kind, email = "corporation", orderer.Value.Email().Value()
		companyName = present(orderer.Value.CompanyName().Value())
		corporateNumber = present(orderer.Value.CorporateNumber().Value())
	}
	orderID, charge := order.Id().Value(), order.Charge()

	if _, err := s.Tx.Exec(`
		INSERT INTO orders (order_id, user_id, subtotal, discount, total, orderer_type, orderer_email,
		                    orderer_name, orderer_company_name, orderer_corporate_number)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		orderID, order.UserId().Value(), charge.Subtotal().Value(), charge.Discount().Value(),
		charge.Total().Value(), kind, email, name, companyName, corporateNumber); err != nil {
		return domain.OrderPlaced{}, err
	}
	insert, err := s.Tx.Prepare(`
		INSERT INTO order_line (order_line_id, order_id, product_id, quantity, unit_price)
		VALUES (?, ?, ?, ?, ?)`)
	if err != nil {
		return domain.OrderPlaced{}, err
	}
	defer insert.Close()
	for _, line := range order.Lines() {
		if _, err := insert.Exec(uuid.NewString(), orderID, line.ProductId().Value(),
			line.Quantity().Value(), line.UnitPrice().Value()); err != nil {
			return domain.OrderPlaced{}, err
		}
	}
	return domain.NewOrderPlaced(r, order)
}

func present(text string) sql.NullString { return sql.NullString{String: text, Valid: true} }
