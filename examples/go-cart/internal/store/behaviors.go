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

// PriceCart is priceCart. It reads every line of the cart, looks each product up to see that it is
// there and on sale, and answers the lines with their prices as a PricedCart. A product that is
// gone or no longer on sale ends it with the model's own case.
//
// The loop that asks for each line's product stays here, in the implementation: the model has no
// traverse, and a fold cannot call another injected behavior.
type PriceCart struct{ Tx *sql.Tx }

func (s PriceCart) Apply(r *model.Run, userID domain.UserId) (domain.PricedCartOrProductNotFoundOrSaleEnded, error) {
	lines, err := s.lines(userID.Value())
	if err != nil {
		return nil, err
	}
	for i := range lines {
		var onSale bool
		err := s.Tx.QueryRow(`SELECT on_sale, price FROM product WHERE product_id = ?`, lines[i].productID).
			Scan(&onSale, &lines[i].unitPrice)
		if errors.Is(err, sql.ErrNoRows) {
			none, err := domain.NewProductNotFound(r)
			return domain.PricedCartOrProductNotFoundOrSaleEndedProductNotFound{Value: none}, err
		}
		if err != nil {
			return nil, err
		}
		if !onSale {
			ended, err := domain.NewSaleEnded(r)
			return domain.PricedCartOrProductNotFoundOrSaleEndedSaleEnded{Value: ended}, err
		}
	}
	priced, err := read(r, pricedCartForm, lines, domain.DecodePricedCart)
	return domain.PricedCartOrProductNotFoundOrSaleEndedPricedCart{Value: priced}, err
}

// lines are the lines of the user's cart, without their prices.
func (s PriceCart) lines(user string) ([]lineRow, error) {
	rows, err := s.Tx.Query(`
		SELECT ci.product_id, ci.quantity
		FROM cart_item ci JOIN cart c ON c.cart_id = ci.cart_id
		WHERE c.user_id = ?
		ORDER BY ci.product_id`, user)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var lines []lineRow
	for rows.Next() {
		var line lineRow
		if err := rows.Scan(&line.productID, &line.quantity); err != nil {
			return nil, err
		}
		lines = append(lines, line)
	}
	return lines, rows.Err()
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
	for _, line := range order.Lines() {
		if _, err := s.Tx.Exec(`
			INSERT INTO order_line (order_line_id, order_id, product_id, quantity, unit_price)
			VALUES (?, ?, ?, ?, ?)`,
			uuid.NewString(), orderID, line.ProductId().Value(), line.Quantity().Value(),
			line.UnitPrice().Value()); err != nil {
			return domain.OrderPlaced{}, err
		}
	}
	return domain.NewOrderPlaced(r, order)
}

func present(text string) sql.NullString { return sql.NullString{String: text, Valid: true} }
